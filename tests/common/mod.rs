//! Shared integration-test fixtures. Every fixture lives in its own canonical
//! temporary directory, so tests are isolated and never touch user state.
#![allow(dead_code)] // each test crate uses a different subset

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

pub struct Fixture {
    _temp: TempDir,
    pub root: PathBuf,
    pub download: PathBuf,
    pub completed: PathBuf,
    pub config: PathBuf,
}

impl Fixture {
    /// `incoming/` and `completed/` plus a `config.xml` pointing at them.
    pub fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        // Canonical, so platform temp symlinks (macOS /var) are not rejected.
        let root = dunce::canonicalize(temp.path()).unwrap();
        let fixture = Self {
            download: root.join("incoming"),
            completed: root.join("completed"),
            config: root.join("config.xml"),
            root,
            _temp: temp,
        };
        fs::create_dir(&fixture.download).unwrap();
        fs::create_dir(&fixture.completed).unwrap();
        fixture.write_config("");
        fixture
    }

    /// Rewrite the config with both bases plus `extra` XML elements.
    pub fn write_config(&self, extra: &str) {
        fs::write(
            &self.config,
            config_xml(&self.download, &self.completed, extra),
        )
        .unwrap();
    }

    /// Create a file (and its parents) under the download base.
    pub fn file(&self, relative: &str, bytes: &[u8]) -> PathBuf {
        let path = self.download.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }

    /// The binary, isolated from the caller's environment and config.
    pub fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_aria_move"));
        command
            .current_dir(&self.root)
            .env("ARIA_MOVE_CONFIG", &self.config)
            .env("NO_COLOR", "1")
            .env_remove("ARIA_MOVE_DISABLE_LOCKS");
        command
    }

    pub fn run(&self, args: &[&dyn AsRef<OsStr>]) -> Output {
        self.command()
            .args(args.iter().map(|a| a.as_ref()))
            .output()
            .unwrap()
    }

    pub fn completed_names(&self) -> Vec<String> {
        names(&self.completed)
    }
}

pub fn config_xml(download: &Path, completed: &Path, extra: &str) -> String {
    let escape = |path: &Path| quick_xml::escape::escape(path.to_str().unwrap()).into_owned();
    format!(
        "<config><download_base>{}</download_base><completed_base>{}</completed_base>{extra}</config>",
        escape(download),
        escape(completed)
    )
}

pub fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Every path under `root` with file contents, for "nothing changed" checks.
pub fn snapshot(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    let mut entries: Vec<_> = walkdir::WalkDir::new(root)
        .into_iter()
        .map(|entry| {
            let entry = entry.unwrap();
            let contents = entry
                .file_type()
                .is_file()
                .then(|| fs::read(entry.path()).unwrap());
            (
                entry.path().strip_prefix(root).unwrap().to_path_buf(),
                contents,
            )
        })
        .collect();
    entries.sort();
    entries
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[track_caller]
pub fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "expected success\nstdout:\n{}\nstderr:\n{}",
        stdout(output),
        stderr(output)
    );
}

#[track_caller]
pub fn assert_failure(output: &Output, message: &str) {
    assert!(
        !output.status.success(),
        "expected failure\nstdout:\n{}",
        stdout(output)
    );
    assert!(
        stderr(output).contains(message),
        "expected {message:?} in stderr:\n{}",
        stderr(output)
    );
}

/// A temporary directory on a different filesystem than the system temp
/// directory, if this machine has one (e.g. `/dev/shm` on Linux, or a target
/// directory on another drive on Windows). Detected by a real rename probe.
pub fn other_filesystem() -> Option<TempDir> {
    let here = tempfile::tempdir().ok()?;
    let probe = here.path().join("probe");
    [env!("CARGO_TARGET_TMPDIR"), "/dev/shm"]
        .into_iter()
        .find_map(|candidate| {
            let there = tempfile::tempdir_in(candidate).ok()?;
            fs::write(&probe, b"").ok()?;
            match fs::rename(&probe, there.path().join("probe")) {
                Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => Some(there),
                _ => None,
            }
        })
}

/// Whether the current Unix user bypasses permission bits.
#[cfg(unix)]
pub fn is_root() -> bool {
    // SAFETY: geteuid has no preconditions and no side effects.
    unsafe { libc::geteuid() == 0 }
}
