use filetime::{set_file_atime, set_file_mtime, FileTime};
use std::fs;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub(super) fn preserve_metadata(dst: &Path, src_meta: &fs::Metadata) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let mode = src_meta.permissions().mode() & 0o777;
        let perms = fs::Permissions::from_mode(mode);
        fs::set_permissions(dst, perms)?;
    }

    let mtime = FileTime::from_last_modification_time(src_meta);
    let atime = FileTime::from_last_access_time(src_meta);
    let _ = set_file_mtime(dst, mtime);
    let _ = set_file_atime(dst, atime);
    Ok(())
}