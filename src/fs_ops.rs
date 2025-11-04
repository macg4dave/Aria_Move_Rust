use anyhow::{bail, Context, Result};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;
use tracing::{info, warn};

use crate::config::Config;
use crate::utils::{unique_destination, ensure_not_base, file_is_mutable, stable_file_probe};

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
    ensure_not_base(&config.download_base, src)?;

    stable_file_probe(src, Duration::from_millis(200), 3)?;

    let dest_dir = &config.completed_base;
    if !config.dry_run {
        fs::create_dir_all(dest_dir).with_context(|| format!("Failed to create destination dir {}", dest_dir.display()))?;
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
            Ok(dest)
        }
        Err(e) => {
            warn!(error = %e, "Atomic rename failed, falling back to copy+remove");
            fs::copy(src, &dest).with_context(|| format!("Copy failed {} -> {}", src.display(), dest.display()))?;
            fs::remove_file(src).with_context(|| format!("Failed to remove original file {}", src.display()))?;
            Ok(dest)
        }
    }
}

/// Move directory contents into completed_base/<src_dir_name>.
pub fn move_dir(config: &Config, src_dir: &Path) -> Result<PathBuf> {
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
                fs::create_dir_all(&new_dir)?;
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
            fs::create_dir_all(parent)?;
        }
        fs::copy(path, &dst).with_context(|| format!("Failed copying {} -> {}", path.display(), dst.display()))?;
        Ok(())
    })?;

    fs::remove_dir_all(src_dir).with_context(|| format!("Failed to remove source directory {}", src_dir.display()))?;
    info!(src = %src_dir.display(), dest = %target.display(), "Copied directory contents and removed source");
    Ok(target)
}

/// Platform hook: try atomic move. This is intentionally small so platform-specific
/// strategies (windows move semantics, replace semantics) can be added by cfg.
fn try_atomic_move(src: &Path, dest: &Path) -> std::io::Result<()> {
    // On most Unixes and Windows rename is atomic when on same FS.
    fs::rename(src, dest)
}

/// Validate the configured paths (wrapper used by CLI).
pub fn validate_paths(cfg: &Config) -> Result<()> {
    cfg.validate()
}