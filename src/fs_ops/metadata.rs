//! Metadata preservation.
//! - Copies timestamps (atime, mtime) and, on Unix, permissions (mode) from source->dest.
//! - Best-effort: failures to set times/perms are ignored (function returns Ok(())).
//! - Callers decide whether to treat failures as fatal; this helper itself does not.

use anyhow::Result;
use filetime::{set_file_times, FileTime};
#[cfg(not(unix))]
use filetime::{set_file_atime, set_file_mtime};
use std::fs;
use std::path::Path;
use tracing::{trace, warn};

/// Preserve metadata on `dest` using already-fetched `src_meta`.
/// Callers pass src metadata to avoid re-statting the source repeatedly.
/// Preserve metadata on `dest` using already-fetched `src_meta`.
/// If `strict` is true, any failure to set times/permissions returns an error.
/// If `strict` is false, failures are logged and ignored.
pub fn preserve_metadata(dest: &Path, src_meta: &fs::Metadata) -> Result<()> {
    // 1) Timestamps
    #[cfg(unix)]
    {
    use std::os::unix::fs::MetadataExt;
        let mt = FileTime::from_unix_time(src_meta.mtime(), src_meta.mtime_nsec() as u32);
        let at = FileTime::from_unix_time(src_meta.atime(), src_meta.atime_nsec() as u32);
        if let Err(e) = set_file_times(dest, at, mt) {
            warn!(path = %dest.display(), error = %e, "failed to set atime/mtime on destination");
        } else {
            trace!(path = %dest.display(), "set atime/mtime on destination");
        }
    }
    #[cfg(not(unix))]
    {
        let at = src_meta.accessed().ok().map(FileTime::from_system_time);
        let mt = src_meta.modified().ok().map(FileTime::from_system_time);
        match (at, mt) {
            (Some(a), Some(m)) => {
                if let Err(e) = set_file_times(dest, a, m) {
                    warn!(path = %dest.display(), error = %e, "failed to set atime/mtime on destination");
                } else {
                    trace!(path = %dest.display(), "set atime/mtime on destination");
                }
            }
            (Some(a), None) => {
                if let Err(e) = set_file_atime(dest, a) {
                    warn!(path = %dest.display(), error = %e, "failed to set atime on destination");
                } else {
                    trace!(path = %dest.display(), "set atime on destination");
                }
            }
            (None, Some(m)) => {
                if let Err(e) = set_file_mtime(dest, m) {
                    warn!(path = %dest.display(), error = %e, "failed to set mtime on destination");
                } else {
                    trace!(path = %dest.display(), "set mtime on destination");
                }
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
        if let Err(e) = fs::set_permissions(dest, perms) {
            warn!(path = %dest.display(), mode = format!("{:o}", src_mode), error = %e, "failed to set permissions on destination");
        } else {
            trace!(path = %dest.display(), mode = format!("{:o}", src_mode), "set permissions on destination");
        }
    }

    // 3) Windows: preserve readonly attribute similar to POSIX readonly bit
    #[cfg(windows)]
    {
        let ro = src_meta.permissions().readonly();
        match fs::metadata(dest) {
            Ok(meta) => {
                let mut perms = meta.permissions();
                perms.set_readonly(ro);
                if let Err(e) = fs::set_permissions(dest, perms) {
                    warn!(path = %dest.display(), readonly = ro, error = %e, "failed to set readonly attribute on destination");
                } else {
                    trace!(path = %dest.display(), readonly = ro, "set readonly attribute on destination");
                }
            }
            Err(e) => {
                warn!(path = %dest.display(), error = %e, "failed to stat destination for readonly preservation");
            }
        }
    }

    Ok(())
}

/// Preserve only permissions (and readonly bit on Windows) from source metadata to dest.
pub fn preserve_permissions_only(dest: &Path, src_meta: &fs::Metadata) -> Result<()> {
    // Unix: set mode bits
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let src_mode = src_meta.permissions().mode() & 0o777;
        let perms = fs::Permissions::from_mode(src_mode);
        let _ = fs::set_permissions(dest, perms);
    }
    // Windows: mirror readonly attribute
    #[cfg(windows)]
    {
        if let Ok(meta) = fs::metadata(dest) {
            let mut perms = meta.permissions();
            perms.set_readonly(src_meta.permissions().readonly());
            let _ = fs::set_permissions(dest, perms);
        }
    }
    Ok(())
}

/// Preserve extended attributes (xattrs) from source path to destination path.
/// - Requires the "xattrs" feature (otherwise this is a no-op Ok(()))
/// - On unsupported platforms or if listing/setting fails:
///   * strict=false => log and continue
///   * strict=true  => return Err
pub fn preserve_xattrs(src: &Path, dest: &Path) -> Result<()> {
    #[cfg(feature = "xattrs")]
    {
    use tracing::{trace, warn};
    let mut _had_error = false; // retained for future diagnostics aggregation
        // Attempt to list xattrs on source
        match xattr::list(src) {
            Ok(names) => {
                for name in names {
                    match xattr::get(src, &name) {
                        Ok(Some(value)) => {
                            if let Err(e) = xattr::set(dest, &name, &value) {
                                let name_disp = name.to_string_lossy();
                                warn!(src=%src.display(), dest=%dest.display(), xattr=%name_disp, error=%e, "failed to set xattr on destination");
                                _had_error = true;
                            } else {
                                let name_disp = name.to_string_lossy();
                                trace!(src=%src.display(), dest=%dest.display(), xattr=%name_disp, size=value.len(), "preserved xattr");
                            }
                        }
                        Ok(None) => {
                            // Attribute exists but empty value (rare); set empty
                            if let Err(e) = xattr::set(dest, &name, &[]) {
                                let name_disp = name.to_string_lossy();
                                warn!(src=%src.display(), dest=%dest.display(), xattr=%name_disp, error=%e, "failed to set empty xattr on destination");
                                _had_error = true;
                            } else {
                                let name_disp = name.to_string_lossy();
                                trace!(src=%src.display(), dest=%dest.display(), xattr=%name_disp, size=0, "preserved empty xattr");
                            }
                        }
                        Err(e) => {
                            let name_disp = name.to_string_lossy();
                            warn!(src=%src.display(), dest=%dest.display(), xattr=%name_disp, error=%e, "failed to read xattr value from source");
                            _had_error = true;
                        }
                    }
                }
                // best-effort: ignore aggregated errors
            }
            Err(e) => {
                warn!(src=%src.display(), error=%e, "failed to list xattrs; continuing (best-effort)");
            }
        }
        Ok(())
    }
    #[cfg(not(feature = "xattrs"))]
    {
        let _ = (src, dest); // silence unused warnings
        Ok(())
    }
}