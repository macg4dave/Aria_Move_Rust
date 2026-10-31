use std::path::{Path, PathBuf};
use std::process::Command;

/// Re-execute exactly one test with an isolated process environment. No unsafe
/// mutations race with the test harness or runtime-created threads.
pub fn config_environment(name: &str, directory: bool, check: impl FnOnce(&Path)) {
    if let Some(root) = std::env::var_os("ARIA_MOVE_TEST_ROOT") {
        check(Path::new(&root));
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root: PathBuf = dunce::canonicalize(temporary.path()).unwrap();
    let config = if directory {
        root.clone()
    } else {
        root.join("config.xml")
    };
    let result = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env("ARIA_MOVE_TEST_ROOT", &root)
        .env("ARIA_MOVE_CONFIG", config)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
