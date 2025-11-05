use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing::debug;

/// Return unique destination by appending timestamp+pid when candidate exists.
pub(crate) fn unique_destination(candidate: &Path) -> PathBuf {
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

    let new_name = format!("{}-{}-{}{}", stem, epoch, pid, ext);
    candidate.with_file_name(new_name)
}

/// Prevent moving the download base itself.
pub(crate) fn ensure_not_base(download_base: &Path, candidate: &Path) -> anyhow::Result<()> {
    let base_real = fs::canonicalize(download_base).unwrap_or_else(|_| download_base.to_path_buf());
    let cand_real = fs::canonicalize(candidate).unwrap_or_else(|_| candidate.to_path_buf());

    if base_real == cand_real {
        Err(anyhow::anyhow!(
            "Refusing to move the download base folder itself: {}",
            download_base.display()
        ))
    } else {
        Ok(())
    }
}

/// Quick writable probe: create and remove a small file.
pub(crate) fn is_writable_probe(dir: &Path) -> std::io::Result<()> {
    let probe = dir.join(format!(".aria_move_probe_{}.tmp", std::process::id()));
    match fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Heuristic to detect if a file is still being written / in-use.
/// - common incomplete suffixes (.part, .aria2, .tmp) -> mutable
/// - if size changes over small interval -> mutable
pub(crate) fn file_is_mutable(path: &Path) -> anyhow::Result<bool> {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        let ext = ext.to_ascii_lowercase();
        if matches!(ext.as_str(), "part" | "aria2" | "tmp" | "crdownload") {
            debug!(
                "File {} has extension {} -> considered mutable",
                path.display(),
                ext
            );
            return Ok(true);
        }
    }

    // Basic stable-size probe
    match stable_file_probe(path, Duration::from_millis(150), 2) {
        Ok(_) => Ok(false),
        Err(_) => Ok(true),
    }
}

/// Probe that waits for `attempts` checks spaced by `interval` where size must be stable.
/// Returns Ok(()) when stable, Err otherwise.
pub(crate) fn stable_file_probe(
    path: &Path,
    interval: Duration,
    attempts: usize,
) -> anyhow::Result<()> {
    let mut last_size = fs::metadata(path)
        .map(|m| m.len())
        .map_err(|e| anyhow::anyhow!(e))?;
    for _ in 0..attempts {
        std::thread::sleep(interval);
        let size = fs::metadata(path)
            .map(|m| m.len())
            .map_err(|e| anyhow::anyhow!(e))?;
        if size == last_size {
            // stable for one interval; consider stable
            return Ok(());
        }
        last_size = size;
    }
    Err(anyhow::anyhow!(
        "File {} did not stabilize in size",
        path.display()
    ))
}
