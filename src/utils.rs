use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing::debug;

/// Return a unique destination by appending timestamp+pid when candidate exists.
/// - Preserves non-UTF8 names (uses OsString).
/// - Format: "<stem>-<millis>-<pid>[ -<n>].<ext?>"
/// - Adds a tiny retry loop if a collision still occurs (extremely unlikely).
pub(crate) fn unique_destination(candidate: &Path) -> PathBuf {
    if !candidate.exists() {
        return candidate.to_path_buf();
    }

    let epoch_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    let pid = std::process::id();

    // Extract stem and extension robustly (handles dotfiles and non-UTF8).
    let stem = candidate
        .file_stem()
        .map(|s| s.to_owned())
        .unwrap_or_else(|| std::ffi::OsStr::new("file").to_owned());
    let ext = candidate.extension().map(|e| e.to_owned());

    // Try base "<stem>-<epoch>-<pid>[.ext]".
    let mut name = std::ffi::OsString::new();
    name.push(&stem);
    name.push(format!("-{epoch_ms}-{pid}"));
    if let Some(ref e) = ext {
        name.push(".");
        name.push(e);
    }
    let mut dest = candidate.with_file_name(&name);
    if !dest.exists() {
        return dest;
    }

    // Fallback attempts: append "-<n>" before the extension.
    for n in 2u32..=5 {
        let mut alt = std::ffi::OsString::new();
        alt.push(&stem);
        alt.push(format!("-{epoch_ms}-{pid}-{n}"));
        if let Some(ref e) = ext {
            alt.push(".");
            alt.push(e);
        }
        dest = candidate.with_file_name(&alt);
        if !dest.exists() {
            return dest;
        }
    }

    // Final fallback with "-final".
    let mut final_name = std::ffi::OsString::new();
    final_name.push(&stem);
    final_name.push(format!("-{epoch_ms}-{pid}-final"));
    if let Some(ref e) = ext {
        final_name.push(".");
        final_name.push(e);
    }
    candidate.with_file_name(final_name)
}

/// Prevent moving the download base itself (exact path equality).
/// Note: intentionally does NOT reject children of the base — callers decide policy.
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

/// Quick writable probe: create and remove a small file in `dir`.
/// Uses create_new to avoid clobbering existing files.
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
/// - Common incomplete suffixes (.part, .aria2, .tmp, .crdownload) -> mutable
/// - If size changes over a short interval -> mutable
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
/// Returns Ok(()) when stable for at least one interval, Err otherwise.
/// Notes:
/// - attempts is the number of re-checks after the initial size read.
/// - Example: attempts=2 -> read, sleep, read (equal -> Ok), else sleep, read (equal -> Ok) else Err.
pub(crate) fn stable_file_probe(
    path: &Path,
    interval: Duration,
    attempts: usize,
) -> anyhow::Result<()> {
    let mut last_size = fs::metadata(path)
        .map(|m| m.len())
        .map_err(anyhow::Error::from)?;
    for _ in 0..attempts {
        std::thread::sleep(interval);
        let size = fs::metadata(path)
            .map(|m| m.len())
            .map_err(anyhow::Error::from)?;
        if size == last_size {
            // Stable for one interval; consider the file quiescent
            return Ok(());
        }
        last_size = size;
    }
    Err(anyhow::anyhow!(
        "File {} did not stabilize in size",
        path.display()
    ))
}
