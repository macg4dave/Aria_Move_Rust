//! Metadata preservation.
//! Optionally copies permissions and timestamps from source to destination.

use anyhow::Result;
use filetime::{set_file_times, FileTime};
use std::fs;
use std::path::Path;

pub(super) fn maybe_preserve_metadata(src: &Path, dest: &Path, preserve: bool) -> Result<()> {
    if !preserve {
        return Ok(());
    }

    let meta =
        fs::metadata(src).map_err(|e| anyhow::anyhow!("stat {} failed: {}", src.display(), e))?;

    let (at_opt, mt_opt) = {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let mt = FileTime::from_unix_time(meta.mtime(), meta.mtime_nsec() as u32);
            let at = FileTime::from_unix_time(meta.atime(), meta.atime_nsec() as u32);
            (Some(at), Some(mt))
        }
        #[cfg(not(unix))]
        {
            let at = meta.accessed().ok().map(FileTime::from_system_time);
            let mt = meta.modified().ok().map(FileTime::from_system_time);
            (at, mt)
        }
    };

    if let (Some(at), Some(mt)) = (at_opt, mt_opt) {
        let _ = set_file_times(dest, at, mt);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(src_meta) = fs::metadata(src) {
            let src_mode = src_meta.permissions().mode() & 0o777;
            if let Ok(dest_meta) = fs::metadata(dest) {
                let mut perms = dest_meta.permissions();
                perms.set_mode(src_mode);
                let _ = fs::set_permissions(dest, perms);
            }
        }
    }

    Ok(())
}
