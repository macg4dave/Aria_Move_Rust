//! Resume / reconciliation pass.
//! Cleans up orphaned resume temp files and removes partial directory copies safely.
//! This runs automatically at startup so headless deployments self-heal after crashes.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use tracing::{debug, warn};

use aria_move::Config; // use public re-export from library crate

// Regex-like simple prefix matching for resume temp files.
fn is_resume_temp(entry: &Path) -> bool {
    if let Some(name) = entry.file_name().and_then(|s| s.to_str()) {
        name.starts_with(".aria_move.resume.") && name.ends_with(".tmp")
    } else {
        false
    }
}

pub fn reconcile(cfg: &Config) -> Result<()> {
    cleanup_resume_temps(&cfg.completed_base)?;
    cleanup_partial_dirs(&cfg.download_base, &cfg.completed_base)?;
    Ok(())
}

fn cleanup_resume_temps(completed_base: &Path) -> Result<()> {
    let rd = match fs::read_dir(completed_base) {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_file() && is_resume_temp(&p) {
            match fs::remove_file(&p) {
                Ok(()) => debug!(path = %p.display(), "Removed orphan resume temp"),
                Err(e) => {
                    warn!(error = %e, path = %p.display(), "Failed to remove orphan resume temp")
                }
            }
        }
    }
    Ok(())
}

fn cleanup_partial_dirs(download_base: &Path, completed_base: &Path) -> Result<()> {
    let rd = match fs::read_dir(completed_base) {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };
    for ent in rd.flatten() {
        let target = ent.path();
        if !target.is_dir() {
            continue;
        }
        // Skip hidden internal dirs.
        if let Some(name) = target.file_name().and_then(|s| s.to_str()) {
            if name.starts_with('.') {
                continue;
            }
        }
        let source = download_base.join(ent.file_name());
        if source.is_dir() {
            // Heuristic: if dest has fewer entries than source, consider it partial and remove.
            let src_count = count_entries(&source).unwrap_or(0);
            let dst_count = count_entries(&target).unwrap_or(u64::MAX); // if error reading, skip
            if src_count > 0 && dst_count < src_count {
                match fs::remove_dir_all(&target) {
                    Ok(()) => {
                        debug!(partial = %target.display(), "Removed partial destination directory for clean restart")
                    }
                    Err(e) => {
                        warn!(error = %e, partial = %target.display(), "Failed to remove partial destination directory")
                    }
                }
            }
        }
    }
    Ok(())
}

fn count_entries(dir: &Path) -> Result<u64> {
    let mut c = 0u64;
    for e in fs::read_dir(dir).with_context(|| format!("read_dir {}", dir.display()))? {
        if e.is_ok() {
            c += 1;
        }
    }
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn removes_orphan_temp() {
        let completed = tempdir().unwrap();
        let download = tempdir().unwrap();
        let tmp = completed
            .path()
            .join(".aria_move.resume.deadbeefdeadbeef.tmp");
        fs::write(&tmp, b"partial").unwrap();
        let cfg = Config {
            download_base: download.path().into(),
            completed_base: completed.path().into(),
            ..Config::default()
        };
        reconcile(&cfg).unwrap();
        assert!(!tmp.exists());
    }

    #[test]
    fn removes_partial_dir() {
        let completed = tempdir().unwrap();
        let download = tempdir().unwrap();
        let src_dir = download.path().join("movie");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("a.bin"), b"a").unwrap();
        fs::write(src_dir.join("b.bin"), b"b").unwrap();
        let dst_dir = completed.path().join("movie");
        fs::create_dir_all(&dst_dir).unwrap();
        fs::write(dst_dir.join("a.bin"), b"a").unwrap();
        let cfg = Config {
            download_base: download.path().into(),
            completed_base: completed.path().into(),
            ..Config::default()
        };
        reconcile(&cfg).unwrap();
        // Partial dest should be gone so move can restart cleanly later.
        assert!(!dst_dir.exists());
    }
}
