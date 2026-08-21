use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use app::ports::{Availability, CommandError, Repository, Staged};
use app::value::{Branch, CommitId};

pub const GIT: &str = "git";

#[derive(Debug, Default)]
pub struct GitCommands;

impl Repository for GitCommands {
    fn root(&self, repo: &Path) -> Result<PathBuf, CommandError> {
        let output = run(repo, &[osstr("rev-parse"), osstr("--show-toplevel")])?;
        Ok(PathBuf::from(single(&output)?))
    }

    fn branch(&self, repo: &Path) -> Result<Option<Branch>, CommandError> {
        let arguments = [
            osstr("symbolic-ref"),
            osstr("--quiet"),
            osstr("--short"),
            osstr("HEAD"),
        ];
        let output = raw(repo, &arguments)?;
        if output.status.code() == Some(1) {
            return Ok(None);
        }
        let output = checked(repo, &arguments, output)?;
        Ok(Some(Branch::new(&single(&output)?).map_err(|source| {
            CommandError::Output {
                command: described(repo, &arguments),
                output: source.to_string(),
            }
        })?))
    }

    fn remote(&self, repo: &Path, name: &str) -> Result<Availability, CommandError> {
        let output = raw(
            repo,
            &[osstr("remote"), osstr("get-url"), OsString::from(name)],
        )?;
        Ok(if output.status.success() {
            Availability::Present
        } else {
            Availability::Missing
        })
    }

    fn head(&self, repo: &Path) -> Result<CommitId, CommandError> {
        let arguments = [osstr("rev-parse"), osstr("HEAD")];
        let output = run(repo, &arguments)?;
        CommitId::new(&single(&output)?).map_err(|source| CommandError::Output {
            command: described(repo, &arguments),
            output: source.to_string(),
        })
    }

    fn stage(&self, repo: &Path, paths: &[&Path]) -> Result<(), CommandError> {
        let mut arguments = vec![osstr("add"), osstr("-A"), osstr("--")];
        arguments.extend(paths.iter().map(|path| path.as_os_str().to_owned()));
        run(repo, &arguments).map(|_| ())
    }

    fn staged(&self, repo: &Path, paths: &[&Path]) -> Result<Staged, CommandError> {
        let mut arguments = vec![
            osstr("diff"),
            osstr("--cached"),
            osstr("--quiet"),
            osstr("--"),
        ];
        arguments.extend(paths.iter().map(|path| path.as_os_str().to_owned()));
        let output = raw(repo, &arguments)?;
        match output.status.code() {
            Some(0) => Ok(Staged::Nothing),
            Some(1) => Ok(Staged::Changes),
            _ => Err(failure(repo, &arguments, &output)),
        }
    }

    fn commit(
        &self,
        repo: &Path,
        message: &str,
        paths: &[&Path],
    ) -> Result<CommitId, CommandError> {
        let mut arguments = vec![
            osstr("commit"),
            osstr("-m"),
            OsString::from(message),
            osstr("--"),
        ];
        arguments.extend(paths.iter().map(|path| path.as_os_str().to_owned()));
        run(repo, &arguments)?;
        self.head(repo)
    }

    fn restore(&self, repo: &Path, commit: &CommitId, paths: &[&Path]) -> Result<(), CommandError> {
        let mut arguments = vec![
            osstr("checkout"),
            OsString::from(commit.as_str()),
            osstr("--"),
        ];
        arguments.extend(paths.iter().map(|path| path.as_os_str().to_owned()));
        run(repo, &arguments).map(|_| ())
    }
}

fn osstr(text: &str) -> OsString {
    OsString::from(text)
}

fn described(repo: &Path, arguments: &[OsString]) -> String {
    let mut command = format!("{GIT} -C {}", repo.display());
    for argument in arguments {
        command.push(' ');
        command.push_str(&argument.to_string_lossy());
    }
    command
}

fn raw(repo: &Path, arguments: &[OsString]) -> Result<Output, CommandError> {
    Command::new(GIT)
        .arg("-C")
        .arg(repo)
        .args(arguments.iter().map(OsString::as_os_str))
        .output()
        .map_err(|source| CommandError::Spawn {
            command: described(repo, arguments),
            source,
        })
}

fn run(repo: &Path, arguments: &[OsString]) -> Result<Output, CommandError> {
    let output = raw(repo, arguments)?;
    checked(repo, arguments, output)
}

fn checked(repo: &Path, arguments: &[OsString], output: Output) -> Result<Output, CommandError> {
    if output.status.success() {
        return Ok(output);
    }
    Err(failure(repo, arguments, &output))
}

fn failure(repo: &Path, arguments: &[OsString], output: &Output) -> CommandError {
    CommandError::Failed {
        command: described(repo, arguments),
        status: output.status.to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn single(output: &Output) -> Result<String, CommandError> {
    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines();
    match (lines.next(), lines.next()) {
        (Some(line), None) => Ok(line.to_owned()),
        _ => Err(CommandError::Output {
            command: GIT.to_owned(),
            output: text.into_owned(),
        }),
    }
}
