use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing::debug;
use crate::shutdown;

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
#[cfg(any(test, feature = "test-helpers"))]
#[allow(dead_code)]
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
        if shutdown::is_requested() {
            return Err(anyhow::anyhow!("interrupted"));
        }
        std::thread::sleep(interval);
        if shutdown::is_requested() {
            return Err(anyhow::anyhow!("interrupted"));
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::io::Write;
    use std::thread;
    use serial_test::serial;

    #[test]
    fn unique_destination_same_when_absent() {
        let td = tempdir().unwrap();
        let p = td.path().join("file.txt");
        assert!(!p.exists());
        let u = unique_destination(&p);
        assert_eq!(u, p);
    }

    #[test]
    fn unique_destination_changes_when_exists() {
        let td = tempdir().unwrap();
        let p = td.path().join("data.bin");
        fs::write(&p, b"x").unwrap();
        let u = unique_destination(&p);
        assert_ne!(u, p);
        // Extension preserved
        assert_eq!(u.extension().and_then(|s| s.to_str()), Some("bin"));
        assert!(!u.exists());
    }

    #[test]
    fn ensure_not_base_matches_fails() {
        let td = tempdir().unwrap();
        let base = td.path().join("base");
        fs::create_dir_all(&base).unwrap();
        let err = ensure_not_base(&base, &base).unwrap_err();
        assert!(format!("{}", err).contains("Refusing to move the download base"));
    }

    #[test]
    #[serial]
    fn stable_file_probe_ok_when_quiescent() {
        shutdown::reset();
        let td = tempdir().unwrap();
        let f = td.path().join("q.txt");
        fs::write(&f, b"abc").unwrap();
        // Use longer interval so background scheduling doesn't race with reset
        stable_file_probe(&f, Duration::from_millis(30), 2).unwrap();
    }

    #[test]
    fn file_is_mutable_when_growing() {
        shutdown::reset();
        let td = tempdir().unwrap();
        let f = td.path().join("grow.log");
        fs::write(&f, b"seed").unwrap();
        // Append in background shortly after
        let f2 = f.clone();
        thread::spawn(move || {
            for _ in 0..3 {
                thread::sleep(Duration::from_millis(120));
                let mut file = fs::OpenOptions::new().append(true).open(&f2).unwrap();
                let _ = writeln!(file, "more");
            }
        });
        let mut_flag = file_is_mutable(&f).unwrap();
        assert!(mut_flag, "should detect mutability while writing");
    }

    #[test]
    fn shutdown_interrupts_probe() {
        shutdown::reset();
        let td = tempdir().unwrap();
        let f = td.path().join("s.txt");
        fs::write(&f, b"abc").unwrap();
        thread::spawn(|| {
            thread::sleep(Duration::from_millis(5));
            shutdown::request_with_reason(1);
        });
        let err = stable_file_probe(&f, Duration::from_millis(20), 5).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("interrupted"));
        shutdown::reset();
    }
}
