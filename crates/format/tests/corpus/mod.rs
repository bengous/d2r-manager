#![allow(
    dead_code,
    reason = "each test binary uses a subset of this shared helper"
)]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub(crate) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

pub(crate) fn fixture_names(directory: &str) -> Vec<String> {
    let path = repo_root().join("fixtures").join(directory);
    let mut names: Vec<String> = std::fs::read_dir(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        .map(|entry| entry.expect("directory entry").file_name())
        .filter(|name| {
            let extension = Path::new(name).extension().and_then(OsStr::to_str);
            matches!(extension, Some("d2s" | "d2i"))
        })
        .filter_map(|name| name.into_string().ok())
        .collect();
    names.sort();
    names
}

pub(crate) fn fixture_bytes(directory: &str, name: &str) -> Vec<u8> {
    let path = repo_root().join("fixtures").join(directory).join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

pub(crate) fn is_stash(name: &str) -> bool {
    Path::new(name).extension().and_then(OsStr::to_str) == Some("d2i")
}
