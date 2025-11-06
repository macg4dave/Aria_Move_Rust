//! Metadata preservation.
//! - Copies timestamps (atime, mtime) and, on Unix, permissions (mode) from source->dest.
//! - Best-effort: failures to set times/perms are ignored (function returns Ok(())).
//! - Callers decide whether to treat failures as fatal; this helper itself does not.

use anyhow::Result;
use filetime::{set_file_atime, set_file_mtime, set_file_times, FileTime};
use std::fs;
use std::path::Path;

/// Preserve metadata on `dest` using already-fetched `src_meta`.
/// Callers pass src metadata to avoid re-statting the source repeatedly.
pub fn preserve_metadata(dest: &Path, src_meta: &fs::Metadata) -> Result<()> {
    // 1) Timestamps
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let mt = FileTime::from_unix_time(src_meta.mtime(), src_meta.mtime_nsec() as u32);
        let at = FileTime::from_unix_time(src_meta.atime(), src_meta.atime_nsec() as u32);
        let _ = set_file_times(dest, at, mt);
    }
    #[cfg(not(unix))]
    {
        let at = src_meta.accessed().ok().map(FileTime::from_system_time);
        let mt = src_meta.modified().ok().map(FileTime::from_system_time);
        match (at, mt) {
            (Some(a), Some(m)) => {
                let _ = set_file_times(dest, a, m);
            }
            (Some(a), None) => {
                let _ = set_file_atime(dest, a);
            }
            (None, Some(m)) => {
                let _ = set_file_mtime(dest, m);
            }
            (None, None) => {}
        }
    }

    // 2) Permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let src_mode = src_meta.permissions().mode() & 0o777;
        let perms = fs::Permissions::from_mode(src_mode);
        let _ = fs::set_permissions(dest, perms);
    }

    Ok(())
}