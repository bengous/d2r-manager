use std::ffi::{OsStr, OsString};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use app::ports::{
    Availability, Clock, CommandError, MirrorPlan, Mirrors, ProcessQuery, Processes, Sighting,
    TimerState, Timers,
};
use app::value::{Timestamp, UnitName, ValueError};

pub const PGREP: &str = "pgrep";
pub const RSYNC: &str = "rsync";
pub const SYSTEMCTL: &str = "systemctl";
pub const STAMP: &str = "%Y-%m-%dT%H:%M:%S%:z";

#[derive(Debug, Default)]
pub struct SystemProcesses;

impl Processes for SystemProcesses {
    fn look_up(&self, query: ProcessQuery<'_>) -> Result<Sighting, CommandError> {
        let (flag, needle) = match query {
            ProcessQuery::ExactName(name) => ("-x", name),
            ProcessQuery::CommandPattern(pattern) => ("-f", pattern),
        };
        let status = Command::new(PGREP)
            .arg(flag)
            .arg(needle)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|source| CommandError::Spawn {
                command: format!("{PGREP} {flag} {needle}"),
                source,
            })?;
        match status.code() {
            Some(0) => Ok(Sighting::Seen),
            Some(1) => Ok(Sighting::Absent),
            _ => Err(CommandError::Failed {
                command: format!("{PGREP} {flag} {needle}"),
                status: status.to_string(),
                stderr: String::new(),
            }),
        }
    }

    fn available(&self, command: &str) -> Result<Availability, CommandError> {
        let Some(path) = std::env::var_os("PATH") else {
            return Ok(Availability::Missing);
        };
        for directory in std::env::split_paths(&path) {
            let candidate = directory.join(command);
            let Ok(metadata) = std::fs::metadata(&candidate) else {
                continue;
            };
            if metadata.is_file() && metadata.permissions().mode() & 0o111 != 0 {
                return Ok(Availability::Present);
            }
        }
        Ok(Availability::Missing)
    }
}

#[derive(Debug, Default)]
pub struct Rsync;

impl Mirrors for Rsync {
    fn mirror(&self, plan: MirrorPlan<'_>) -> Result<(), CommandError> {
        let arguments = rsync_arguments(plan);
        let output = Command::new(RSYNC)
            .args(&arguments)
            .output()
            .map_err(|source| CommandError::Spawn {
                command: RSYNC.to_owned(),
                source,
            })?;
        finished(RSYNC, output).map(|_| ())
    }
}

#[must_use]
pub fn rsync_arguments(plan: MirrorPlan<'_>) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = vec![
        "--archive".into(),
        "--delete".into(),
        "--delete-excluded".into(),
    ];
    for pattern in plan.excluded {
        arguments.push(format!("--exclude={pattern}").into());
    }
    arguments.push("--".into());
    arguments.push(inside(plan.source));
    arguments.push(inside(plan.target));
    arguments
}

fn inside(path: &Path) -> OsString {
    let mut text = path.as_os_str().to_owned();
    text.push(OsStr::new("/"));
    text
}

#[derive(Debug, Default)]
pub struct SystemdTimers;

impl Timers for SystemdTimers {
    fn state(&self, unit: &UnitName) -> Result<TimerState, CommandError> {
        let output = systemctl(&["show", unit.as_str(), "--property=LoadState,ActiveState"])?;
        let text = String::from_utf8_lossy(&output.stdout);
        let mut load = None;
        let mut active = None;
        for line in text.lines() {
            match line.split_once('=') {
                Some(("LoadState", value)) => load = Some(value.trim().to_owned()),
                Some(("ActiveState", value)) => active = Some(value.trim().to_owned()),
                _ => {}
            }
        }
        let (Some(load), Some(active)) = (load, active) else {
            return Err(CommandError::Output {
                command: format!("{SYSTEMCTL} --user show {unit}"),
                output: text.into_owned(),
            });
        };
        if load == "not-found" {
            return Ok(TimerState::Absent);
        }
        Ok(if active == "active" {
            TimerState::Active
        } else {
            TimerState::Inactive
        })
    }

    fn pause(&self, unit: &UnitName) -> Result<(), CommandError> {
        systemctl(&["stop", unit.as_str()]).map(|_| ())
    }

    fn resume(&self, unit: &UnitName) -> Result<(), CommandError> {
        systemctl(&["start", unit.as_str()]).map(|_| ())
    }
}

fn systemctl(arguments: &[&str]) -> Result<Output, CommandError> {
    let command = format!("{SYSTEMCTL} --user {}", arguments.join(" "));
    let output = Command::new(SYSTEMCTL)
        .arg("--user")
        .args(arguments)
        .output()
        .map_err(|source| CommandError::Spawn {
            command: command.clone(),
            source,
        })?;
    finished(&command, output)
}

fn finished(command: &str, output: Output) -> Result<Output, CommandError> {
    if output.status.success() {
        return Ok(output);
    }
    Err(CommandError::Failed {
        command: command.to_owned(),
        status: output.status.to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Result<Timestamp, ValueError> {
        Timestamp::new(&jiff::Zoned::now().strftime(STAMP).to_string())
    }
}
