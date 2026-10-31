//! Source protection and quiescence probes shared by filesystem operations.
use crate::shutdown;
use anyhow::Context;
use std::fs;
use std::path::Path;
use std::time::Duration;

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

/// Incomplete extensions are never treated as completed downloads.
pub(crate) fn file_is_mutable(path: &Path) -> anyhow::Result<bool> {
    if path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
        matches!(
            s.to_ascii_lowercase().as_str(),
            "part" | "aria2" | "tmp" | "crdownload"
        )
    }) {
        return Ok(true);
    }
    stable_file_probe(path, Duration::from_millis(50), 3)?;
    Ok(false)
}

/// Require unchanged size AND modification time across a sampling interval.
/// This is only a quiescence heuristic; copy verification checks the bytes too.
pub(crate) fn stable_file_probe(
    path: &Path,
    interval: Duration,
    attempts: usize,
) -> anyhow::Result<()> {
    probe_with(path, attempts, || std::thread::sleep(interval))
}

fn probe_with(path: &Path, attempts: usize, mut pause: impl FnMut()) -> anyhow::Result<()> {
    let read = || -> anyhow::Result<_> {
        let meta = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
        Ok((meta.len(), meta.modified()?))
    };
    let mut last = read()?;
    for _ in 0..attempts {
        shutdown::check()?;
        pause();
        shutdown::check()?;
        let current = read()?;
        if current == last {
            return Ok(());
        }
        last = current;
    }
    anyhow::bail!("File {} did not stabilize", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_changed_and_cancelled_probes() -> anyhow::Result<()> {
        let _guard = crate::shutdown::test_guard();
        let td = tempfile::tempdir()?;
        let path = td.path().join("data");
        fs::write(&path, b"seed")?;
        probe_with(&path, 1, || {})?;
        assert!(probe_with(&path, 1, || fs::write(&path, b"longer").unwrap()).is_err());
        let error = probe_with(&path, 1, shutdown::request).unwrap_err();
        shutdown::reset();
        assert_eq!(
            error.downcast_ref::<crate::AriaMoveError>(),
            Some(&crate::AriaMoveError::Interrupted)
        );
        Ok(())
    }
    #[test]
    fn base_itself_is_rejected() -> anyhow::Result<()> {
        let td = tempfile::tempdir()?;
        assert!(ensure_not_base(td.path(), td.path()).is_err());
        Ok(())
    }
}
