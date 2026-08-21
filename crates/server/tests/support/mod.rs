#![allow(dead_code, reason = "each test binary drives a subset of this harness")]

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::Value;

use app::ports::{
    Availability, Clock, CommandError, ProcessQuery, Processes, Sighting, TimerState, Timers,
};
use app::value::{Timestamp, UnitName, ValueError};
use infra::files::{FileLock, RealFiles};
use infra::git::GitCommands;
use infra::system::Rsync;
use server::{Server, Services};

pub(crate) const AT: &str = "2026-08-21T02:13:19+02:00";
pub(crate) const ABSENT_TIMER: &str = "d2r-manager-tests-no-such.timer";
pub(crate) const STASH: &str = "linux";

static COUNTER: AtomicU32 = AtomicU32::new(0);

#[derive(Debug)]
pub(crate) struct TempTree {
    root: PathBuf,
}

impl TempTree {
    pub(crate) fn new(label: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "d2r-manager-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("temporary tree");
        Self { root }
    }

    pub(crate) fn directory(&self, relative: &str) -> PathBuf {
        let path = self.root.join(relative);
        std::fs::create_dir_all(&path).expect("directory");
        path
    }

    pub(crate) fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub(crate) fn git(repo: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(arguments)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {}: {}",
        arguments.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_owned()
}

pub(crate) fn repository(tree: &TempTree, name: &str) -> PathBuf {
    let repo = tree.directory(name);
    let origin = tree.directory(&format!("{name}-origin.git"));
    Command::new("git")
        .arg("init")
        .arg("--bare")
        .arg("-b")
        .arg("main")
        .arg(&origin)
        .output()
        .expect("git init --bare");
    Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .arg(&repo)
        .output()
        .expect("git init");
    git(&repo, &["config", "user.email", "tests@example.invalid"]);
    git(&repo, &["config", "user.name", "d2r-manager tests"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    git(
        &repo,
        &["remote", "add", "origin", &origin.to_string_lossy()],
    );
    std::fs::write(repo.join("README.md"), "backup repository\n").expect("seed file");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-m", "init"]);
    repo
}

pub(crate) fn copy_fixture(relative: &str, target: &Path) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .canonicalize()
        .expect("fixtures");
    std::fs::copy(root.join(relative), target).expect("fixture copy");
}

pub(crate) fn seed(saves: &Path) {
    std::fs::create_dir_all(saves).expect("saves directory");
    copy_fixture(
        "v105/linux/ModernSharedStashSoftCoreV2.d2i",
        &saves.join(format!("{STASH}.d2i")),
    );
    copy_fixture("v105/linux/Gous.d2s", &saves.join("Gous.d2s"));
}

pub(crate) fn encoded(path: &Path) -> String {
    let mut text = String::new();
    for byte in path.to_string_lossy().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                text.push(char::from(byte));
            }
            other => {
                let _ = write!(text, "%{other:02X}");
            }
        }
    }
    text
}

pub(crate) fn bytes_of(directory: &Path) -> Vec<(String, Vec<u8>)> {
    let mut held: Vec<(String, Vec<u8>)> = std::fs::read_dir(directory)
        .expect("the saves list")
        .map(|entry| {
            let entry = entry.expect("directory entry");
            (
                entry.file_name().to_string_lossy().into_owned(),
                std::fs::read(entry.path()).expect("save file"),
            )
        })
        .collect();
    held.sort();
    held
}

#[derive(Debug, Default)]
pub(crate) struct StillProcesses {
    seen: RefCell<BTreeSet<String>>,
}

impl StillProcesses {
    pub(crate) fn sees(&self, query: &str) -> &Self {
        self.seen.borrow_mut().insert(query.to_owned());
        self
    }
}

impl Processes for StillProcesses {
    fn look_up(&self, query: ProcessQuery<'_>) -> Result<Sighting, CommandError> {
        let key = match query {
            ProcessQuery::ExactName(name) => format!("-x {name}"),
            ProcessQuery::CommandPattern(pattern) => format!("-f {pattern}"),
        };
        Ok(if self.seen.borrow().contains(&key) {
            Sighting::Seen
        } else {
            Sighting::Absent
        })
    }

    fn available(&self, _command: &str) -> Result<Availability, CommandError> {
        Ok(Availability::Present)
    }
}

#[derive(Debug, Default)]
pub(crate) struct RecordedTimers {
    pub(crate) calls: RefCell<Vec<String>>,
}

impl RecordedTimers {
    pub(crate) fn called(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl Timers for RecordedTimers {
    fn state(&self, _unit: &UnitName) -> Result<TimerState, CommandError> {
        self.calls.borrow_mut().push("state".to_owned());
        Ok(TimerState::Active)
    }

    fn pause(&self, _unit: &UnitName) -> Result<(), CommandError> {
        self.calls.borrow_mut().push("pause".to_owned());
        Ok(())
    }

    fn resume(&self, _unit: &UnitName) -> Result<(), CommandError> {
        self.calls.borrow_mut().push("resume".to_owned());
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct SteadyClock;

impl Clock for SteadyClock {
    fn now(&self) -> Result<Timestamp, ValueError> {
        Timestamp::new(AT)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Call {
    pub(crate) method: &'static str,
    pub(crate) path: &'static str,
    pub(crate) target: String,
    pub(crate) body: Option<String>,
    pub(crate) origin: Option<String>,
    pub(crate) host: Option<String>,
}

impl Call {
    pub(crate) fn get(path: &'static str, query: &str) -> Self {
        Self {
            method: "GET",
            path,
            target: if query.is_empty() {
                path.to_owned()
            } else {
                format!("{path}?{query}")
            },
            body: None,
            origin: None,
            host: None,
        }
    }

    pub(crate) fn post(path: &'static str, body: &str) -> Self {
        Self {
            method: "POST",
            path,
            target: path.to_owned(),
            body: Some(body.to_owned()),
            origin: None,
            host: None,
        }
    }

    pub(crate) fn with_body(mut self, body: &str) -> Self {
        self.body = Some(body.to_owned());
        self
    }

    pub(crate) fn spoken_by(mut self, method: &'static str) -> Self {
        self.method = method;
        self
    }

    pub(crate) fn with_origin(mut self, origin: &str) -> Self {
        self.origin = Some(origin.to_owned());
        self
    }

    pub(crate) fn with_host(mut self, host: &str) -> Self {
        self.host = Some(host.to_owned());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Answer {
    pub(crate) status: u16,
    pub(crate) content_type: String,
    pub(crate) body: String,
    pub(crate) call: Call,
}

impl Answer {
    pub(crate) fn json(&self) -> Value {
        serde_json::from_str(&self.body)
            .unwrap_or_else(|error| panic!("{}: {error}: {}", self.status, self.body))
    }

    pub(crate) fn kind(&self) -> String {
        self.json()["error"]["kind"]
            .as_str()
            .unwrap_or_else(|| panic!("no error kind in {}", self.body))
            .to_owned()
    }
}

#[derive(Debug)]
pub(crate) struct Rig {
    pub(crate) tree: TempTree,
    pub(crate) processes: StillProcesses,
    pub(crate) timers: RecordedTimers,
    server: Server,
    files: RealFiles,
    locks: FileLock,
    mirrors: Rsync,
    repository: GitCommands,
    clock: SteadyClock,
}

impl Rig {
    pub(crate) fn new(label: &str) -> Self {
        Self {
            tree: TempTree::new(label),
            processes: StillProcesses::default(),
            timers: RecordedTimers::default(),
            server: Server::bind(0).expect("the server binds a free loopback port"),
            files: RealFiles,
            locks: FileLock,
            mirrors: Rsync,
            repository: GitCommands,
            clock: SteadyClock,
        }
    }

    pub(crate) fn address(&self) -> SocketAddr {
        self.server.address()
    }

    pub(crate) fn services(&self) -> Services<'_> {
        Services {
            files: &self.files,
            locks: &self.locks,
            processes: &self.processes,
            mirrors: &self.mirrors,
            repository: &self.repository,
            timers: &self.timers,
            clock: &self.clock,
        }
    }

    pub(crate) fn ask_together(&self, first: Call, second: Call) -> [Answer; 2] {
        let address = self.server.address();
        let asked = [first, second];
        let clients = asked.clone().map(|call| {
            std::thread::spawn(move || {
                let (status, content_type, body) = send(address, &call);
                Answer {
                    status,
                    content_type,
                    body,
                    call,
                }
            })
        });
        for _ in &asked {
            self.server
                .serve_once(&self.services())
                .expect("the server answers");
        }
        clients.map(|client| client.join().expect("the client reads the answer"))
    }

    pub(crate) fn ask(&self, call: Call) -> Answer {
        let address = self.server.address();
        let asked = call.clone();
        let client = std::thread::spawn(move || send(address, &asked));
        self.server
            .serve_once(&self.services())
            .expect("the server answers");
        let (status, content_type, body) = client.join().expect("the client reads the answer");
        Answer {
            status,
            content_type,
            body,
            call,
        }
    }
}

fn send(address: SocketAddr, call: &Call) -> (u16, String, String) {
    let mut stream = TcpStream::connect(address).expect("the client connects");
    let host = call.host.clone().unwrap_or_else(|| address.to_string());
    let mut request = format!(
        "{} {} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n",
        call.method, call.target
    );
    if let Some(origin) = &call.origin {
        let _ = writeln!(request, "Origin: {origin}\r");
    }
    if let Some(body) = &call.body {
        let _ = writeln!(
            request,
            "Content-Type: application/json\r\nContent-Length: {}\r",
            body.len()
        );
    }
    request.push_str("\r\n");
    if let Some(body) = &call.body {
        request.push_str(body);
    }
    stream
        .write_all(request.as_bytes())
        .expect("the request leaves");
    stream.flush().expect("the request leaves");
    let mut answer = Vec::new();
    stream
        .read_to_end(&mut answer)
        .expect("the answer comes back");
    read(&answer)
}

fn read(answer: &[u8]) -> (u16, String, String) {
    let split = find(answer, b"\r\n\r\n")
        .unwrap_or_else(|| panic!("no header ends this answer: {}", show(answer)));
    let head = String::from_utf8_lossy(&answer[..split]).into_owned();
    let body = &answer[split + 4..];

    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("no status opens this answer: {head}"));
    let header = |name: &str| {
        head.lines()
            .skip(1)
            .find_map(|line| {
                let (field, value) = line.split_once(':')?;
                field.eq_ignore_ascii_case(name).then(|| value.trim())
            })
            .unwrap_or_default()
            .to_owned()
    };
    let body = if header("transfer-encoding").eq_ignore_ascii_case("chunked") {
        dechunked(body)
    } else {
        body.to_vec()
    };
    (
        status,
        header("content-type"),
        String::from_utf8_lossy(&body).into_owned(),
    )
}

fn dechunked(mut data: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    loop {
        let end = find(data, b"\r\n").expect("a chunk names its size");
        let size = usize::from_str_radix(String::from_utf8_lossy(&data[..end]).trim(), 16)
            .expect("a chunk size reads as hexadecimal");
        if size == 0 {
            return body;
        }
        let start = end + 2;
        body.extend_from_slice(&data[start..start + size]);
        data = &data[start + size + 2..];
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn show(answer: &[u8]) -> String {
    String::from_utf8_lossy(answer).into_owned()
}
