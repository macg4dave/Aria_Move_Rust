use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn unique_temp_path(dst_dir: &Path) -> PathBuf {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_name = format!(".aria_move.{}.{}.tmp", pid, nanos);
    let mut p = dst_dir.to_path_buf();
    p.push(tmp_name);
    p
}

pub(super) fn is_cross_device(e: &io::Error) -> bool {
    // std::io::ErrorKind has no CrossDeviceLink variant on stable platforms,
    // so detect EXDEV / ERROR_NOT_SAME_DEVICE via raw OS error codes.
    if let Some(code) = e.raw_os_error() {
        #[cfg(unix)]
        {
            // EXDEV
            if code == 18 {
                return true;
            }
        }
        #[cfg(windows)]
        {
            // ERROR_NOT_SAME_DEVICE
            if code == 17 {
                return true;
            }
        }
    }
    false
}

#[cfg(unix)]
pub(super) fn fsync_dir(dir: &Path) -> io::Result<()> {
    let f = File::open(dir)?;
    f.sync_all()
}

#[cfg(windows)]
pub(super) fn fsync_dir(_dir: &Path) -> io::Result<()> {
    Ok(())
}