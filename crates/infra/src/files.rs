use std::cell::RefCell;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use app::ports::{FileError, Files, Lease, Locks, PathState};

#[derive(Debug, Default)]
pub struct RealFiles;

impl Files for RealFiles {
    fn probe(&self, path: &Path) -> Result<PathState, FileError> {
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => Ok(PathState::Symlink),
            Ok(metadata) if metadata.is_dir() => Ok(PathState::Directory),
            Ok(_) => Ok(PathState::File),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(PathState::Missing),
            Err(source) => Err(failure(path, "probe", source)),
        }
    }

    fn real_path(&self, path: &Path) -> Result<PathBuf, FileError> {
        std::fs::canonicalize(path).map_err(|source| failure(path, "resolve", source))
    }

    fn create_directory(&self, path: &Path) -> Result<(), FileError> {
        std::fs::create_dir_all(path).map_err(|source| failure(path, "create", source))
    }

    fn entries(&self, path: &Path) -> Result<Vec<PathBuf>, FileError> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path).map_err(|source| failure(path, "list", source))? {
            let entry = entry.map_err(|source| failure(path, "list", source))?;
            entries.push(entry.path());
        }
        entries.sort();
        Ok(entries)
    }

    fn read(&self, path: &Path) -> Result<Vec<u8>, FileError> {
        std::fs::read(path).map_err(|source| failure(path, "read", source))
    }

    fn write(&self, path: &Path, data: &[u8]) -> Result<(), FileError> {
        std::fs::write(path, data).map_err(|source| failure(path, "write", source))
    }

    fn append_line(&self, path: &Path, line: &str) -> Result<(), FileError> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|source| failure(path, "open", source))?;
        writeln!(file, "{line}").map_err(|source| failure(path, "append", source))
    }
}

#[derive(Debug, Default)]
pub struct FileLock {
    held: RefCell<Option<File>>,
}

impl Locks for FileLock {
    fn hold(&self, path: &Path) -> Result<Lease, FileError> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path)
            .map_err(|source| failure(path, "open", source))?;
        match file.try_lock() {
            Ok(()) => {
                *self.held.borrow_mut() = Some(file);
                Ok(Lease::Held)
            }
            Err(TryLockError::WouldBlock) => Ok(Lease::Busy),
            Err(TryLockError::Error(source)) => Err(failure(path, "lock", source)),
        }
    }
}

fn failure(path: &Path, action: &'static str, source: std::io::Error) -> FileError {
    FileError {
        path: path.to_owned(),
        action,
        source,
    }
}
