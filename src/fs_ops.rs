use anyhow::{bail, Result};
use filetime::{FileTime, set_file_times};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::process;
use std::fs::OpenOptions;
use std::io;
use tracing::{info, warn};
use walkdir::WalkDir;
use fs2::FileExt;
#[cfg(unix)]
use libc;

use crate::config::Config;
use crate::utils::{unique_destination, ensure_not_base, file_is_mutable, stable_file_probe};
use crate::shutdown;

/// Resolve the source path. If `maybe_path` is Some and exists, that wins.
/// Otherwise find the newest file under download_base modified within recent_window.
pub fn resolve_source_path(config: &Config, maybe_path: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = maybe_path {
        if p.exists() {
            return Ok(p.to_path_buf());
        }
        warn!("Provided source path does not exist: {}", p.display());
    }

    let cutoff = SystemTime::now()
        .checked_sub(config.recent_window)
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let newest = WalkDir::new(&config.download_base)
        .min_depth(1)
        .max_depth(4)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok().and_then(|m| m.modified().ok().map(|t| (t, e.into_path()))))
        .filter(|(modified, _)| *modified >= cutoff)
        .max_by_key(|(modified, _)| *modified);

    newest
        .map(|(_, p)| p)
        .ok_or_else(|| anyhow::anyhow!("No recently modified file found under {}", config.download_base.display()))
}

/// Top-level move entry: file or directory.
pub fn move_entry(config: &Config, src: &Path) -> Result<PathBuf> {
    ensure_not_base(&config.download_base, src)?;

    if src.is_file() {
        if file_is_mutable(src)? {
            bail!("Source file '{}' appears to be in-use or still being written", src.display());
        }
        move_file(config, src)
    } else if src.is_dir() {
        move_dir(config, src)
    } else {
        bail!("Source path is neither file nor directory: {}", src.display())
    }
}

/// Move a single file into `completed_base`.
pub fn move_file(config: &Config, src: &Path) -> Result<PathBuf> {
    // abort early if shutdown requested
    if shutdown::is_requested() {
        bail!("shutdown requested");
    }

    // Acquire per-source lock
    let _move_lock = acquire_move_lock(src)?;

    ensure_not_base(&config.download_base, src)?;

    stable_file_probe(src, Duration::from_millis(200), 3)?;

    let dest_dir = &config.completed_base;
    if !config.dry_run {
        fs::create_dir_all(dest_dir).map_err(io_error_with_help("create destination directory", dest_dir))?;
    } else {
        info!(action = "mkdir -p", path = %dest_dir.display(), "dry-run");
    }

    let file_name = src
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Source file missing a file name: {}", src.display()))?;
    let mut dest = dest_dir.join(file_name);

    if dest.exists() {
        dest = unique_destination(&dest);
    }

    if config.dry_run {
        info!(src = %src.display(), dest = %dest.display(), "dry-run: would move file");
        return Ok(dest);
    }

    match try_atomic_move(src, &dest) {
        Ok(()) => {
            info!(src = %src.display(), dest = %dest.display(), "Renamed file atomically");
            maybe_preserve_metadata(src, &dest, config.preserve_metadata)?;
            Ok(dest)
        }
        Err(e) => {
            // Extract underlying io::Error (if present) to produce better hints.
            #[cfg(unix)]
            let hint: &str = match e.downcast_ref::<io::Error>().and_then(|ioe| ioe.raw_os_error()) {
                Some(code) if code == libc::EXDEV => "cross-filesystem; will copy instead",
                Some(code) if code == libc::EACCES || code == libc::EPERM => "permission denied; check destination perms",
                _ => "falling back to copy",
            };

            #[cfg(not(unix))]
            let hint: &str = match e.downcast_ref::<io::Error>().map(|ioe| ioe.kind()) {
                Some(io::ErrorKind::PermissionDenied) => "permission denied; check destination perms",
                _ => "falling back to copy",
            };

            warn!(error = %e, hint, "Atomic rename failed, using safe copy+rename");
            safe_copy_and_rename_with_metadata(src, &dest, config.preserve_metadata)?;
            fs::remove_file(src).map_err(io_error_with_help("remove original file", src))?;
            Ok(dest)
        }
    }
}

/// Move directory contents into completed_base/<src_dir_name>.
pub fn move_dir(config: &Config, src_dir: &Path) -> Result<PathBuf> {
    // abort early if shutdown requested
    if shutdown::is_requested() {
        bail!("shutdown requested");
    }

    // Acquire per-source lock
    let _move_lock = acquire_move_lock(src_dir)?;

    ensure_not_base(&config.download_base, src_dir)?;
    let src_name = src_dir
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Source directory missing name: {}", src_dir.display()))?;
    let target = config.completed_base.join(src_name);

    if config.dry_run {
        info!(src = %src_dir.display(), dest = %target.display(), "dry-run: would move directory");
        return Ok(target);
    }

    // Try fast rename first (works on same filesystem).
    if fs::rename(src_dir, &target).is_ok() {
        info!(src = %src_dir.display(), dest = %target.display(), "Renamed directory atomically");
        return Ok(target);
    }

    // otherwise copy files; create target tree
    WalkDir::new(src_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_dir())
        .try_for_each(|d| -> Result<()> {
            if let Ok(rel) = d.path().strip_prefix(src_dir) {
                let new_dir = target.join(rel);
                fs::create_dir_all(&new_dir).map_err(io_error_with_help("create directory", &new_dir))?;
            }
            Ok(())
        })?;

    let files: Vec<_> = WalkDir::new(src_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();

    files.par_iter().try_for_each(|path| -> Result<()> {
        // skip files that look in-use
        if file_is_mutable(path)? {
            return Err(anyhow::anyhow!("File '{}' seems in-use; aborting directory move", path.display()));
        }
        let rel = path.strip_prefix(src_dir)?;
        let dst = target.join(rel);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).map_err(io_error_with_help("create directory", parent))?;
        }
        fs::copy(path, &dst).map_err(io_error_with_help("copy file to destination", &dst))?;
        Ok(())
    })?;

    fs::remove_dir_all(src_dir).map_err(io_error_with_help("remove source directory", src_dir))?;
    info!(src = %src_dir.display(), dest = %target.display(), "Copied directory contents and removed source");
    Ok(target)
}

/// Platform hook: try atomic move. Return anyhow::Result so callers can attach context/hints.
fn try_atomic_move(src: &Path, dest: &Path) -> Result<()> {
    // On most Unixes and Windows rename is atomic when on same FS.
    #[cfg(windows)]
    {
        // On Windows std::fs::rename does not overwrite dest — remove it first.
        if dest.exists() {
            fs::remove_file(dest).map_err(io_error_with_help("remove existing destination before rename", dest))?;
        }
    }
    // perform rename; map io errors to anyhow with helpful hints
    fs::rename(src, dest).map_err(io_error_with_help("rename source to destination", dest))?;

    let dest_dir = dest.parent().unwrap();
    let dirf = OpenOptions::new().read(true).open(dest_dir).map_err(io_error_with_help("open destination directory for sync", dest_dir))?;
    dirf.sync_all().map_err(io_error_with_help("fsync destination directory", dest_dir))?;

    Ok(())
}

/// Validate the configured paths (wrapper used by CLI).
pub fn validate_paths(cfg: &Config) -> Result<()> {
    cfg.validate()
}

/// Copy src -> temp-in-dest-dir, fsync temp file, rename temp -> dest, fsync parent dir.
/// This mitigates TOCTOU races and ensures the destination is durable when the function returns.
pub fn safe_copy_and_rename(src: &Path, dest: &Path) -> Result<()> {
    let dest_dir = dest.parent().ok_or_else(|| anyhow::anyhow!("Destination has no parent: {}", dest.display()))?;

    // ensure dest_dir exists
    fs::create_dir_all(dest_dir).map_err(io_error_with_help("create destination directory", dest_dir))?;

    // create a unique temporary path in the destination directory
    let pid = process::id();
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let tmp_name = format!(".aria_move.tmp.{}.{}", pid, now);
    let tmp_path = dest_dir.join(&tmp_name);

    // copy to temp path
    fs::copy(src, &tmp_path).map_err(io_error_with_help("copy to temporary file", &tmp_path))?;

    // open temp and sync data to disk
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&tmp_path)
        .map_err(io_error_with_help("open temporary file for sync", &tmp_path))?;
    f.sync_all().map_err(io_error_with_help("fsync temporary file", &tmp_path))?;

    // atomically rename temp -> dest
    #[cfg(windows)]
    {
        // Windows won't atomically replace an existing file; remove it first to allow rename.
        if dest.exists() {
            fs::remove_file(dest).map_err(io_error_with_help("remove existing destination before rename", dest))?;
        }
    }
    fs::rename(&tmp_path, dest).map_err(io_error_with_help("rename temporary file to destination", dest))?;

    // sync destination directory to persist the rename
    let dirf = OpenOptions::new()
        .read(true)
        .open(dest_dir)
        .map_err(io_error_with_help("open destination directory for sync", dest_dir))?;
    dirf.sync_all().map_err(io_error_with_help("fsync destination directory", dest_dir))?;

    Ok(())
}

/// Same as safe_copy_and_rename, but optionally preserves src permissions and mtime on dest.
pub fn safe_copy_and_rename_with_metadata(src: &Path, dest: &Path, preserve: bool) -> Result<()> {
    safe_copy_and_rename(src, dest)?;
    maybe_preserve_metadata(src, dest, preserve)?;
    Ok(())
}

/// Conditionally copy permissions (unix) and mtime from src -> dest.
pub fn maybe_preserve_metadata(src: &Path, dest: &Path, preserve: bool) -> Result<()> {
    if !preserve {
        return Ok(());
    }

    // gather source metadata
    let meta = match fs::metadata(src) {
        Ok(m) => m,
        Err(e) => return Err(anyhow::anyhow!("stat {} failed: {}", src.display(), e)),
    };

    // compute access & modify times in a platform-appropriate way
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

    // Permissions: unix-only (copy mode bits)
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

/// Guard that holds an exclusive lock on a sidecar lock file.
/// The lock is released when this value is dropped. We also try to remove the
/// lock file on drop (best-effort).
struct MoveLock {
    _file: std::fs::File, // renamed to silence dead_code warning; retained for RAII
    path: PathBuf,
}

impl Drop for MoveLock {
    fn drop(&mut self) {
        // File lock is released automatically when file is dropped.
        // Best-effort: remove the lock file. Ignore errors.
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Acquire an exclusive advisory lock for the given source path.
/// This prevents concurrent instances from moving the same source at once.
fn acquire_move_lock(src: &Path) -> Result<MoveLock> {
    // Create a sidecar lock file next to the source:
    //   - for files:  file.ext -> file.ext.aria_move.lock
    //   - for dirs:   dir -> dir.aria_move.lock
    let lock_path = {
        let mut p = src.to_path_buf();
        let ext = match p.extension() {
            Some(e) => format!("{}.aria_move.lock", e.to_string_lossy()),
            None => "aria_move.lock".to_string(),
        };
        p.set_extension(ext);
        p
    };

    // Ensure parent directory exists (it should if src exists)
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    // Open/create the lock file and try to lock it exclusively.
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false) // ensure we don't accidentally truncate an existing lock file
        .open(&lock_path)
        .map_err(io_error_with_help("open lock file", &lock_path))?;

    match file.try_lock_exclusive() {
        Ok(()) => Ok(MoveLock { _file: file, path: lock_path }),
        Err(e) => {
            // WouldBlock => already locked by another process
            if e.kind() == std::io::ErrorKind::WouldBlock {
                bail!(
                    "Another aria_move process is operating on '{}'; try again later",
                    src.display()
                );
            }
            Err(anyhow::anyhow!("failed to acquire lock for '{}': {} — is another process running?", src.display(), e))
        }
    }
}

/// Turn a low-level io::Error into an actionable message with context and hints.
fn io_error_with_help<'a>(op: &'a str, path: &'a Path) -> impl FnOnce(io::Error) -> anyhow::Error + 'a {
    move |e: io::Error| {
        let mut msg = format!("{} '{}': {}", op, path.display(), e);
        // Enrich with platform-specific hints
        if let Some(code) = e.raw_os_error() {
            #[cfg(unix)]
            {
                match code {
                    libc::EACCES | libc::EPERM => {
                        msg.push_str(" — permission denied; check ownership and write permissions.");
                    }
                    libc::EXDEV => {
                        msg.push_str(" — cross-filesystem; atomic rename not possible.");
                    }
                    libc::EBUSY => {
                        msg.push_str(" — resource busy; ensure no other process is writing.");
                    }
                    libc::ENOENT => {
                        msg.push_str(" — path not found; verify it exists.");
                    }
                    _ => {}
                }
            }
        } else {
            match e.kind() {
                io::ErrorKind::PermissionDenied => {
                    msg.push_str(" — permission denied; check ownership and write permissions.");
                }
                io::ErrorKind::NotFound => {
                    msg.push_str(" — path not found; verify it exists.");
                }
                io::ErrorKind::AlreadyExists => {
                    msg.push_str(" — already exists; remove or choose a unique name.");
                }
                _ => {}
            }
        }
        anyhow::anyhow!(msg)
    }
}

// Usage in move_file fallback:
//
// match try_atomic_move(src, &dest) {
//   Ok(()) => { ... }
//   Err(e) => {
//       warn!(error = %e, "Atomic rename failed, falling back to safe copy+rename");
//       safe_copy_and_rename_with_metadata(src, &dest, config.preserve_metadata)?;
//       // remove original src after successful copy+rename
//       fs::remove_file(src).with_context(|| format!("Failed to remove original file {}", src.display()))?;
//       Ok(dest)
//   }
// }