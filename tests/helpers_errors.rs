use std::io;
use std::path::Path;

use aria_move::fs_ops::{io_error_with_help, io_error_with_help_io};

#[test]
fn notfound_fallback_hint_includes_path() {
    let p = Path::new("/nonexistent/path/for/test");
    let f = io_error_with_help("open", p);
    let err = f(io::Error::from(io::ErrorKind::NotFound));
    let msg = format!("{}", err);
    assert!(msg.contains("open"));
    assert!(msg.contains(p.to_string_lossy().as_ref()));
    assert!(msg.contains("path not found"));
}

#[cfg(unix)]
#[test]
fn enospc_hint_present() {
    let p = Path::new("/tmp");
    let f = io_error_with_help("write", p);
    let err = f(io::Error::from_raw_os_error(libc::ENOSPC));
    let msg = format!("{}", err);
    assert!(msg.contains("insufficient space"), "msg was: {}", msg);
    assert!(msg.contains("os code"), "should include os code in message");
}

#[cfg(unix)]
#[test]
fn erofs_hint_present() {
    let p = Path::new("/tmp");
    let f = io_error_with_help("write", p);
    let err = f(io::Error::from_raw_os_error(libc::EROFS));
    let msg = format!("{}", err);
    assert!(msg.contains("read-only filesystem"), "msg was: {}", msg);
}

#[cfg(unix)]
#[test]
fn test_loop_and_name_too_long_hints() {
    // These may not be triggerable on all platforms but message generation is deterministic.
    let eloop = io::Error::from_raw_os_error(libc::ELOOP);
    let nametoolong = io::Error::from_raw_os_error(libc::ENAMETOOLONG);
    let p = Path::new("/tmp");
    let f = io_error_with_help("op", p);
    let m1 = format!("{}", f(eloop));
    let f = io_error_with_help("op", p);
    let m2 = format!("{}", f(nametoolong));
    assert!(m1.contains("symlink cycle"));
    assert!(m2.contains("too long"));
}

#[cfg(unix)]
#[test]
fn test_fd_limit_hints() {
    let emfile = io::Error::from_raw_os_error(libc::EMFILE);
    let enfile = io::Error::from_raw_os_error(libc::ENFILE);
    let p = Path::new("/tmp");
    let f = io_error_with_help("op", p);
    let m1 = format!("{}", f(emfile));
    let f = io_error_with_help("op", p);
    let m2 = format!("{}", f(enfile));
    assert!(m1.contains("descriptor limit"));
    assert!(m2.contains("file table"));
}

#[test]
fn io_adapter_preserves_kind() {
    let p = Path::new("/tmp/test.txt");
    let f = io_error_with_help_io("create", p);
    let e = io::Error::from(io::ErrorKind::AlreadyExists);
    let wrapped = f(e);
    assert_eq!(wrapped.kind(), io::ErrorKind::AlreadyExists);
    let msg = format!("{}", wrapped);
    assert!(msg.contains("already exists"));
}
