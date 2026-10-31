//! Native primitives for Linux and macOS (and best-effort other Unix systems).
use crate::logging::Severity;
use std::ffi::CString;
use std::fs::{self, DirBuilder, File, Metadata, OpenOptions};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::Path;

fn c_path(path: &Path) -> io::Result<CString> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "path contains an interior NUL byte",
        )
    })
}

pub(crate) fn is_link(meta: &Metadata) -> bool {
    meta.file_type().is_symlink()
}

pub(crate) fn file_identity(_file: &File, meta: &Metadata) -> io::Result<(u64, u64)> {
    Ok((meta.dev(), meta.ino()))
}

pub(crate) fn free_space(path: &Path) -> io::Result<u64> {
    let path = c_path(path)?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: `path` is NUL-terminated and `stat` is writable storage for the result.
    if unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: statvfs succeeded and initialized the structure.
    let stat = unsafe { stat.assume_init() };
    let block = if stat.f_frsize == 0 {
        stat.f_bsize
    } else {
        stat.f_frsize
    };
    #[allow(
        clippy::useless_conversion,
        reason = "statvfs field widths differ by target (32-bit counts on macOS)"
    )]
    Ok(u64::from(stat.f_bavail).saturating_mul(u64::from(block)))
}

pub(crate) fn sync_directory(path: &Path) -> io::Result<()> {
    let directory = super::open_directory(path)?;
    directory_sync_result(path, directory.sync_all())
}

fn directory_sync_result(path: &Path, result: io::Result<()>) -> io::Result<()> {
    match result {
        // Some mounted filesystems cannot flush directory handles. File data
        // is still synced before publication; actual storage errors stay fatal.
        Err(error) if matches!(error.raw_os_error(), Some(libc::EINVAL | libc::ENOTSUP)) => {
            tracing::debug!(path = %path.display(), %error, "filesystem does not support directory sync");
            Ok(())
        }
        result => result,
    }
}

/// Lock only this download's inode, never its parent or destination directory.
pub(crate) fn try_lock_source(path: &Path) -> io::Result<Option<File>> {
    let file = if fs::symlink_metadata(path)?.is_dir() {
        super::open_directory(path)?
    } else {
        super::open_regular_read(path)?
    };
    try_lock(file)
}

fn try_lock(file: File) -> io::Result<Option<File>> {
    use std::os::fd::AsRawFd;
    // SAFETY: `file` owns a valid descriptor for the duration of the call.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        return Ok(Some(file));
    }
    let error = io::Error::last_os_error();
    if error.kind() == io::ErrorKind::WouldBlock {
        Ok(None)
    } else {
        Err(error)
    }
}

pub(super) fn configure_read(options: &mut OpenOptions) {
    // O_NONBLOCK keeps a FIFO from blocking the open; it is rejected right after.
    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
}

pub(super) fn configure_directory(options: &mut OpenOptions) {
    options.custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW);
}

pub(super) fn configure_log(options: &mut OpenOptions) {
    options
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
}

/// Send one record to syslog (journald under systemd, the unified log on macOS).
pub(crate) fn system_log(severity: Severity, message: &str) {
    let Ok(message) = CString::new(message.replace('\0', " ")) else {
        return;
    };
    let priority = match severity {
        Severity::Error => libc::LOG_ERR,
        Severity::Warning => libc::LOG_WARNING,
    };
    // SAFETY: the identifier is a static C string, and the "%s" format
    // consumes exactly the one NUL-terminated argument passed.
    unsafe {
        libc::openlog(c"aria_move".as_ptr(), libc::LOG_PID, libc::LOG_DAEMON);
        libc::syslog(priority, c"%s".as_ptr(), message.as_ptr());
    }
}

pub(super) fn configure_private_dir(builder: &mut DirBuilder) {
    builder.mode(0o700);
}

pub(super) fn prepare_removal(_path: &Path) -> io::Result<()> {
    Ok(())
}

pub(super) fn rename_noreplace(src: &Path, dst: &Path) -> io::Result<()> {
    match native_rename_noreplace(src, dst) {
        // The filesystem (e.g. NFS, older ZFS, some FUSE mounts) lacks the flag.
        Err(e)
            if matches!(
                e.raw_os_error(),
                Some(libc::EINVAL | libc::ENOSYS | libc::ENOTSUP)
            ) =>
        {
            claim_then_rename(src, dst)
        }
        result => result,
    }
}

#[cfg(target_os = "linux")]
fn native_rename_noreplace(src: &Path, dst: &Path) -> io::Result<()> {
    let (from, to) = (c_path(src)?, c_path(dst)?);
    // SAFETY: both strings are NUL-terminated and outlive the call.
    let rc = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "macos")]
fn native_rename_noreplace(src: &Path, dst: &Path) -> io::Result<()> {
    let (from, to) = (c_path(src)?, c_path(dst)?);
    // SAFETY: both strings are NUL-terminated and outlive the call.
    if unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn native_rename_noreplace(_src: &Path, _dst: &Path) -> io::Result<()> {
    Err(io::Error::from_raw_os_error(libc::ENOTSUP))
}

/// Exclusive publication without a no-replace rename flag: claim the
/// destination name atomically, then move into the claim.
/// - files: `link` fails if the name exists; the old name is then unlinked.
/// - directories: `mkdir` claims the name; `rename` may replace only an
///   empty directory, which is the one just created.
fn claim_then_rename(src: &Path, dst: &Path) -> io::Result<()> {
    if fs::symlink_metadata(src)?.is_dir() {
        fs::create_dir(dst)?;
        fs::rename(src, dst).inspect_err(|_| {
            let _ = fs::remove_dir(dst);
        })
    } else {
        fs::hard_link(src, dst)?;
        fs::remove_file(src).inspect_err(|_| {
            let _ = fs::remove_file(dst);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_sync_tolerates_only_unsupported_operations() {
        let path = Path::new("share");
        for code in [libc::EINVAL, libc::ENOTSUP] {
            assert!(directory_sync_result(path, Err(io::Error::from_raw_os_error(code))).is_ok());
        }
        for code in [libc::EIO, libc::ENOSPC, libc::EACCES, libc::EROFS] {
            assert_eq!(
                directory_sync_result(path, Err(io::Error::from_raw_os_error(code)))
                    .unwrap_err()
                    .raw_os_error(),
                Some(code)
            );
        }
    }

    #[test]
    fn claim_fallback_publishes_files_and_directories_exclusively() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let (file, file_dst) = (root.join("file"), root.join("file.out"));
        fs::write(&file, b"bytes").unwrap();
        fs::write(&file_dst, b"existing").unwrap();
        assert_eq!(
            claim_then_rename(&file, &file_dst).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read(&file_dst).unwrap(), b"existing");
        fs::remove_file(&file_dst).unwrap();
        claim_then_rename(&file, &file_dst).unwrap();
        assert_eq!(fs::read(&file_dst).unwrap(), b"bytes");
        assert!(!file.exists());

        let (dir, dir_dst) = (root.join("dir"), root.join("dir.out"));
        fs::create_dir_all(dir.join("child")).unwrap();
        fs::create_dir(&dir_dst).unwrap();
        assert!(claim_then_rename(&dir, &dir_dst).is_err());
        assert!(dir.join("child").is_dir(), "source untouched on refusal");
        fs::remove_dir(&dir_dst).unwrap();
        claim_then_rename(&dir, &dir_dst).unwrap();
        assert!(dir_dst.join("child").is_dir());
        assert!(!dir.exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cross_device_rename_reports_crosses_devices() {
        // /dev/shm is a separate tmpfs on Linux; skip where it is unavailable.
        let Ok(other) = tempfile::tempdir_in("/dev/shm") else {
            return;
        };
        let here = tempfile::tempdir().unwrap();
        if fs::metadata(here.path()).unwrap().dev() == fs::metadata(other.path()).unwrap().dev() {
            return;
        }
        let src = here.path().join("f");
        fs::write(&src, b"x").unwrap();
        let error = rename_noreplace(&src, &other.path().join("f")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::CrossesDevices);
        assert!(src.exists());
    }
}
