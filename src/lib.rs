//! Core library for `aria_move`.
//!
//! This contains the main logic (config, resolving source path, moving files/directories).
//! The CLI in `src/main.rs` wires this to arguments and logging.

use anyhow::{bail, Context, Result};
use rayon::prelude::*;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

/// Default constants - change these at compile-time or provide a config wrapper.
pub const DOWNLOAD_BASE_DEFAULT: &str = "/mnt/World/incoming";
pub const COMPLETED_BASE_DEFAULT: &str = "/mnt/World/completed";
pub const RECENT_FILE_WINDOW: Duration = Duration::from_secs(5 * 60);

/// Configuration holder for base paths and time window.
#[derive(Debug, Clone)]
pub struct Config {
    pub download_base: PathBuf,
    pub completed_base: PathBuf,
    pub recent_window: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            download_base: PathBuf::from(DOWNLOAD_BASE_DEFAULT),
            completed_base: PathBuf::from(COMPLETED_BASE_DEFAULT),
            recent_window: RECENT_FILE_WINDOW,
        }
    }
}

impl Config {
    /// Construct config with explicit values.
    pub fn new(download_base: impl Into<PathBuf>, completed_base: impl Into<PathBuf>, recent_window: Duration) -> Self {
        Self {
            download_base: download_base.into(),
            completed_base: completed_base.into(),
            recent_window,
        }
    }
}

/// Resolve the source path. If `maybe_path` is Some and exists, that wins.
/// If not provided or doesn't exist, we look for a recently modified file under `download_base`.
pub fn resolve_source_path(config: &Config, maybe_path: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = maybe_path {
        if p.exists() {
            return Ok(p.to_path_buf());
        }
        tracing::warn!("Provided source path doesn't exist: {}", p.display());
    }

    let cutoff = SystemTime::now()
        .checked_sub(config.recent_window)
        .unwrap_or(SystemTime::UNIX_EPOCH);

    // Stream walk and pick the newest recent file (avoid building and sorting a large Vec)
    let newest = WalkDir::new(&config.download_base)
        .min_depth(1)
        .max_depth(4)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            e.metadata()
                .ok()
                .and_then(|m| m.modified().ok().map(|t| (t, e.into_path())))
        })
        .filter(|(modified, _)| *modified >= cutoff)
        .max_by_key(|(modified, _)| *modified);

    newest
        .map(|(_, p)| p)
        .ok_or_else(|| anyhow::anyhow!("Could not find a recently modified file under {}", config.download_base.display()))
}

/// The main "move" entry point. Decides if `src` is file or dir and delegates.
/// Will refuse to move the download base directory itself.
pub fn move_entry(config: &Config, src: &Path) -> Result<PathBuf> {
    ensure_not_base(&config.download_base, src)?;

    if src.is_file() {
        let dest = move_file(config, src)?;
        Ok(dest)
    } else if src.is_dir() {
        let dest = move_dir(config, src)?;
        Ok(dest)
    } else {
        bail!("Source path does not exist or is neither file nor directory: {}", src.display())
    }
}

/// Move a single file into `completed_base`.
/// Returns the final destination path.
pub fn move_file(config: &Config, src: &Path) -> Result<PathBuf> {
    ensure_not_base(&config.download_base, src)?;
    let dest_dir = &config.completed_base;
    fs::create_dir_all(dest_dir).with_context(|| format!("Failed to create destination dir {}", dest_dir.display()))?;

    let file_name = src
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Source file missing a file name: {}", src.display()))?;
    let mut dest_path = dest_dir.join(file_name);

    if dest_path.exists() {
        dest_path = unique_destination(&dest_path);
    }

    // Try to rename atomically; fall back to copy+remove on cross-device or other errors.
    match try_atomic_move(src, &dest_path) {
        Ok(()) => {
            tracing::info!(src = %src.display(), dest = %dest_path.display(), "Renamed file atomically");
            Ok(dest_path)
        }
        Err(e) => {
            tracing::warn!(error = %e, "Atomic rename failed, falling back to copy+remove");
            fs::copy(src, &dest_path).with_context(|| format!("Copy failed {} -> {}", src.display(), dest_path.display()))?;
            fs::remove_file(src).with_context(|| format!("Failed to remove original file {}", src.display()))?;
            Ok(dest_path)
        }
    }
}

/// Move a directory's contents into completed_base/<source_dir_name>.
/// Returns the target directory path.
pub fn move_dir(config: &Config, src_dir: &Path) -> Result<PathBuf> {
    ensure_not_base(&config.download_base, src_dir)?;
    let src_name = src_dir
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Source directory missing name: {}", src_dir.display()))?;
    let target_base = config.completed_base.join(src_name);

    fs::create_dir_all(&target_base)
        .with_context(|| format!("Failed to create target directory {}", target_base.display()))?;

    // Try atomic move of whole directory first — works if same fs.
    if fs::rename(src_dir, &target_base).is_ok() {
        tracing::info!(src = %src_dir.display(), dest = %target_base.display(), "Renamed directory atomically");
        return Ok(target_base);
    }

    // Fallback: create dirs first (streaming) then copy files in parallel.
    WalkDir::new(src_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_dir())
        .for_each(|d| {
            if let Ok(rel) = d.path().strip_prefix(src_dir) {
                let new_dir = target_base.join(rel);
                let _ = fs::create_dir_all(&new_dir);
            }
        });

    let files: Vec<_> = WalkDir::new(src_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();

    files.par_iter().try_for_each(|path| -> Result<()> {
        let rel = path.strip_prefix(src_dir)?;
        let dst = target_base.join(rel);

        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(path, &dst).with_context(|| format!("Failed copying {} -> {}", path.display(), dst.display()))?;
        Ok(())
    })?;

    fs::remove_dir_all(src_dir).with_context(|| format!("Failed to remove source directory {}", src_dir.display()))?;
    tracing::info!(src = %src_dir.display(), dest = %target_base.display(), "Copied directory contents and removed source");
    Ok(target_base)
}

/// Attempt to rename src -> dest. Returns Err if unable (e.g., cross-device).
fn try_atomic_move(src: &Path, dest: &Path) -> io::Result<()> {
    fs::rename(src, dest)
}

/// If the candidate already exists, generates a new unique path by appending timestamp+pid.
fn unique_destination(candidate: &Path) -> PathBuf {
    if !candidate.exists() {
        return candidate.to_path_buf();
    }

    let epoch = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();

    let pid = std::process::id();
    let stem = candidate
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = candidate
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();

    // produce name like "name-<epoch>-<pid>.ext"
    let new_name = format!("{}-{}-{}{}", stem, epoch, pid, ext);
    candidate.with_file_name(new_name)
}

/// Ensure we are not being asked to move the download base itself.
fn ensure_not_base(download_base: &Path, candidate: &Path) -> Result<()> {
    // Direct equality check; also canonicalize to be safer in presence of symlinks.
    let base_canon = fs::canonicalize(download_base).unwrap_or_else(|_| download_base.to_path_buf());
    let cand_canon = fs::canonicalize(candidate).unwrap_or_else(|_| candidate.to_path_buf());

    if base_canon == cand_canon {
        bail!("Refusing to move the download base folder itself: {}", download_base.display())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn unique_destination_makes_new_name_when_exists() {
        let dir = assert_fs::TempDir::new().unwrap();
        let p = dir.child("exists.txt");
        p.touch().unwrap();
        let candidate = p.path().to_path_buf();
        let new = unique_destination(&candidate);
        assert_ne!(candidate, new);
    }

    #[test]
    fn move_file_success() {
        let temp = assert_fs::TempDir::new().unwrap();
        let download = temp.child("incoming");
        let completed = temp.child("completed");
        download.create_dir_all().unwrap();
        completed.create_dir_all().unwrap();

        let source = download.child("a.txt");
        source.write_str("hello").unwrap();

        let cfg = Config::new(download.path().to_path_buf(), completed.path().to_path_buf(), RECENT_FILE_WINDOW);
        let dest = move_file(&cfg, source.path()).expect("move_file should succeed");

        assert!(dest.exists());
        assert!(!source.path().exists());
        let content = std::fs::read_to_string(&dest).unwrap();
        assert_eq!(content, "hello");
    }

    #[test]
    fn move_dir_success() {
        let temp = assert_fs::TempDir::new().unwrap();
        let download = temp.child("incoming");
        let completed = temp.child("completed");
        download.create_dir_all().unwrap();
        completed.create_dir_all().unwrap();

        // create nested files
        let d = download.child("folder");
        d.create_dir_all().unwrap();
        let f1 = d.child("one.txt");
        f1.write_str("one").unwrap();
        let sub = d.child("sub");
        sub.create_dir_all().unwrap();
        let f2 = sub.child("two.txt");
        f2.write_str("two").unwrap();

        let cfg = Config::new(download.path().to_path_buf(), completed.path().to_path_buf(), RECENT_FILE_WINDOW);
        let src_dir = d.path();
        let dest = move_dir(&cfg, src_dir).expect("move_dir should succeed");

        assert!(dest.exists());
        assert!(dest.join("one.txt").exists());
        assert!(dest.join("sub").join("two.txt").exists());
        assert!(!src_dir.exists());
    }
}
