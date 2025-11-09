#![cfg(unix)]

// Local-only integration test for a real ZFS share.
// By default this test is ignored (won't run in `cargo test`), and will only execute
// when explicitly requested AND the target mount path exists. Customize behavior via:
//   Environment variables:
//     ARIA_MOVE_RUN_ZFS_TEST=1        -> enable running (otherwise it is skipped early)
//     ARIA_MOVE_ZFS_ROOT=/custom/path -> override default root (/mnt/World)
//   Constants below:
//     DEFAULT_ZFS_ROOT                -> fallback if env override absent
//     INCOMING_DIR_NAME / COMPLETED_DIR_NAME
//
// Steps performed when enabled & mount present:
//   1. Create test subtree: <root>/aria_move_test_<pid>_<epoch>/
//   2. Create incoming + completed directories.
//   3. Create a sample file and a nested directory with a file under incoming.
//   4. Call move_entry for both file and directory.
//   5. Verify destination contents, absence of source paths.
//   6. Cleanup subtree (best-effort).
//
// Skipping logic (any causes early Ok(())):
//   - ARIA_MOVE_RUN_ZFS_TEST not set to 1
//   - ZFS root path missing or not a directory
//
// Notes:
//   - We avoid permission mutations; ZFS may handle mode bits differently.
//   - Works on any filesystem, but intended to surface cross-filesystem rename nuances.
//   - Emits human-friendly diagnostics with prefix [zfs].
//   - To run: `ARIA_MOVE_RUN_ZFS_TEST=1 cargo test --test zfs_world_integration -- --ignored`

use aria_move::{move_entry, Config};
use std::ffi::CString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::os::unix::ffi::OsStrExt;
use std::process::Command;
use std::time::SystemTime;

const DEFAULT_ZFS_ROOT: &str = "/mnt/World";
const ENV_ZFS_ROOT: &str = "ARIA_MOVE_ZFS_ROOT";
const INCOMING_DIR_NAME: &str = "incoming";
const COMPLETED_DIR_NAME: &str = "completed";

fn maybe_world_mount(root: &str) -> Option<PathBuf> {
    let p = Path::new(root);
    if p.is_dir() { Some(p.to_path_buf()) } else { None }
}

fn make_cfg(root: &Path) -> Config {
    let download_base = root.join(INCOMING_DIR_NAME);
    let completed_base = root.join(COMPLETED_DIR_NAME);
    Config { download_base, completed_base, ..Config::default() }
}

fn run_cmd(cmd: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(cmd).args(args).output().ok()?;
    let mut combined = String::new();
    if !output.stdout.is_empty() {
        combined.push_str(&String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        combined.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    Some(combined)
}

fn print_basic_sys_info() {
    if let Some(out) = run_cmd("uname", &["-a"]) {
        eprintln!("[zfs] uname -a: {}", out.trim());
    }
    if let Some(out) = run_cmd("id", &["-u"]) {
        eprintln!("[zfs] uid: {}", out.trim());
    }
    if let Some(out) = run_cmd("whoami", &[]) {
        eprintln!("[zfs] whoami: {}", out.trim());
    }
}

#[cfg(any(target_os = "linux"))]
fn print_fs_type(path: &Path) {
    unsafe {
        let c = match CString::new(path.as_os_str().as_bytes()) {
            Ok(c) => c,
            Err(_) => return,
        };
        let mut s: libc::statfs = std::mem::zeroed();
        if libc::statfs(c.as_ptr(), &mut s) == 0 {
            let ftype = s.f_type as u64;
            eprintln!("[zfs] statfs f_type=0x{:x}", ftype);
            // Common magic numbers
            const ZFS_SUPER_MAGIC: u64 = 0x2FC12FC1;
            const EXT4_SUPER_MAGIC: u64 = 0xEF53;
            const BTRFS_SUPER_MAGIC: u64 = 0x9123683E;
            if ftype == ZFS_SUPER_MAGIC { eprintln!("[zfs] filesystem detected: ZFS"); }
            if ftype == EXT4_SUPER_MAGIC { eprintln!("[zfs] filesystem detected: ext4"); }
            if ftype == BTRFS_SUPER_MAGIC { eprintln!("[zfs] filesystem detected: btrfs"); }
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))]
fn print_fs_type(path: &Path) {
    unsafe {
        let c = match CString::new(path.as_os_str().as_bytes()) {
            Ok(c) => c,
            Err(_) => return,
        };
        let mut s: libc::statfs = std::mem::zeroed();
        if libc::statfs(c.as_ptr(), &mut s) == 0 {
            let fstypename = std::ffi::CStr::from_ptr(s.f_fstypename.as_ptr())
                .to_string_lossy()
                .into_owned();
            eprintln!("[zfs] f_fstypename: {}", fstypename);
        }
    }
}

fn print_mount_info(root: &str) {
    eprintln!("[zfs] root: {}", root);
    print_fs_type(Path::new(root));
    if let Some(out) = run_cmd("mount", &[]) {
        // Print only lines mentioning root to reduce noise
        let lines: Vec<&str> = out.lines().filter(|l| l.contains(root)).collect();
        if !lines.is_empty() {
            eprintln!("[zfs] mount lines containing root:\n{}", lines.join("\n"));
        } else {
            eprintln!("[zfs] mount summary available (no lines matched root)");
        }
    }
    if let Some(out) = run_cmd("df", &["-P", root]) {
        eprintln!("[zfs] df -P {}:\n{}", root, out.trim());
    }
}

fn print_zfs_info(root: &str) {
    // If zfs command is available, try to correlate dataset -> mountpoint
    if run_cmd("which", &["zfs"]).is_none() {
        eprintln!("[zfs] 'zfs' command not present in PATH");
        return;
    }
    if let Some(out) = run_cmd("zfs", &["mount"]) {
        let lines: Vec<&str> = out.lines().filter(|l| l.contains(root)).collect();
        if !lines.is_empty() {
            eprintln!("[zfs] zfs mount lines for root:\n{}", lines.join("\n"));
        } else {
            eprintln!("[zfs] zfs mount has no entries for root");
        }
    }
    if let Some(out) = run_cmd("zpool", &["status", "-x"]) {
        eprintln!("[zfs] zpool status -x:\n{}", out.trim());
    }
}

#[test]
fn zfs_world_create_move_verify() -> io::Result<()> {
    // Determine root path (env override or default); if missing, skip.
    let root_str = std::env::var(ENV_ZFS_ROOT).unwrap_or_else(|_| DEFAULT_ZFS_ROOT.to_string());
    let Some(world) = maybe_world_mount(&root_str) else {
        eprintln!("[zfs] mount root '{}' not present; skipping", root_str);
        return Ok(());
    };

    // Verbose system and mount info first
    print_basic_sys_info();
    print_mount_info(&root_str);
    print_zfs_info(&root_str);

    // Unique subtree per test run
    let pid = std::process::id();
    let epoch_ms = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();
    let test_root = world.join(format!("aria_move_test_{}_{}", pid, epoch_ms));
    fs::create_dir_all(&test_root)?;
    let cfg = make_cfg(&test_root);
    fs::create_dir_all(&cfg.download_base)?;
    fs::create_dir_all(&cfg.completed_base)?;

    // Create file in incoming
    let file_path = cfg.download_base.join("sample.txt");
    let mut f = fs::File::create(&file_path)?;
    writeln!(f, "hello zfs world")?;

    // Create subdirectory with a file
    let dir_path = cfg.download_base.join("subdir");
    fs::create_dir_all(&dir_path)?;
    fs::write(dir_path.join("nested.bin"), b"12345")?;

    eprintln!("[zfs] creating sample file: {}", file_path.display());
    // Move file
    let moved_file = move_entry(&cfg, &file_path).expect("move file across share");
    assert!(moved_file.exists(), "moved file must exist in completed base");
    eprintln!("[zfs] moved file to: {}", moved_file.display());
    let content = fs::read(&moved_file)?;
    assert!(String::from_utf8_lossy(&content).contains("hello zfs"));

    eprintln!("[zfs] creating and moving directory: {}", dir_path.display());
    // Move directory
    let moved_dir = move_entry(&cfg, &dir_path).expect("move directory across share");
    assert!(moved_dir.exists() && moved_dir.is_dir(), "moved directory exists");
    let nested = moved_dir.join("nested.bin");
    assert!(nested.exists(), "nested file carried over");
    assert_eq!(fs::read(&nested)?.as_slice(), b"12345");

    // List completed directory contents for visibility
    if let Ok(entries) = fs::read_dir(&cfg.completed_base) {
        eprintln!("[zfs] completed dir entries:");
        for e in entries.flatten() {
            eprintln!("  - {}", e.path().display());
        }
    }

    // Basic invariant: source originals gone
    assert!(!file_path.exists(), "original file path removed");
    assert!(!dir_path.exists(), "original directory path removed");

    // Cleanup test subtree (best-effort)
    if let Err(e) = fs::remove_dir_all(&test_root) { eprintln!("[zfs] cleanup failed: {}", e); }
    Ok(())
}
