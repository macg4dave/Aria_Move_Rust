//! File lifecycle: validate, lock, exclusive rename or verified staged copy.
use super::atomic::{MoveOutcome, try_atomic_move};
use super::copy::Snapshot;
use crate::{Config, shutdown};
use anyhow::{Context, Result, ensure};
use std::fs;
use std::path::{Path, PathBuf};

/// Move one regular file. Dry runs never create directories or lock files.
pub fn move_file(config: &Config, src: &Path) -> Result<PathBuf> {
    move_file_impl(config, src, false)
}

fn move_file_impl(config: &Config, src: &Path, force_copy: bool) -> Result<PathBuf> {
    super::entry::validate_source(config, src, false)?;
    let candidate = config
        .completed_base
        .join(src.file_name().context("source has no file name")?);
    if config.dry_run {
        return Ok(super::duplicate::unique_destination(&candidate));
    }
    crate::config::paths::create_directory(&config.completed_base)?;
    let _locks =
        super::lock::acquire_move_locks(src, &config.completed_base, config.disable_locks)?;
    super::entry::validate_source(config, src, false)?;
    crate::utils::stable_file_probe(src, std::time::Duration::from_millis(50), 3)?;
    let dest = super::duplicate::unique_destination(&candidate);
    if !force_copy && try_atomic_move(src, &dest)? == MoveOutcome::Renamed {
        return Ok(dest); // Rename carries metadata and xattrs without re-reading a vanished source.
    }
    let size = fs::metadata(src)?.len();
    super::space::ensure_space_for_copy(&config.completed_base, size)?;
    let mut stage = tempfile::Builder::new()
        .prefix(".aria_move.copy.")
        .tempfile_in(&config.completed_base)?;
    let stage_path = stage.path().to_path_buf();
    let (snapshot, _) = super::copy::copy_to_file(
        src,
        stage.as_file_mut(),
        config.preserve_metadata,
        config.preserve_permissions,
        &stage_path,
    )?;
    shutdown::check()?;
    crate::platform::rename_noreplace(stage.path(), &dest)?;
    drop(stage);
    crate::platform::sync_directory(&config.completed_base)?;
    // Publication is permanent. Any subsequent failure retains both copies for inspection.
    verify_source(src, &dest, &snapshot)?;
    shutdown::check()?;
    fs::remove_file(src).with_context(|| {
        format!(
            "destination published at {}; remove source {}",
            dest.display(),
            src.display()
        )
    })?;
    if let Some(parent) = src.parent() {
        crate::platform::sync_directory(parent)?;
    }
    Ok(dest)
}

pub(super) fn verify_source(src: &Path, dest: &Path, snapshot: &Snapshot) -> Result<()> {
    let mut source = crate::platform::open_regular_read(src)?;
    snapshot.check(&source)?;
    let mut destination = crate::platform::open_regular_read(dest)?;
    super::io_copy::verify_equal(&mut source, &mut destination)?;
    snapshot.check(&source)?;
    ensure!(
        Snapshot::read(&crate::platform::open_regular_read(src)?)? == *snapshot,
        "source path identity changed"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copy_fallback_ignores_untrusted_resume_and_preserves_metadata() -> Result<()> {
        let _guard = crate::shutdown::test_guard();
        let td = tempfile::tempdir()?;
        let incoming = td.path().join("in");
        let completed = td.path().join("out");
        fs::create_dir(&incoming)?;
        fs::create_dir(&completed)?;
        let mut cfg = Config::new(&incoming, &completed);
        cfg.preserve_metadata = true;
        let src = incoming.join("a");
        fs::write(&src, b"correct")?;
        let stale = super::super::util::resume_temp_path(&completed.join("a"));
        fs::write(&stale, b"invalid")?;
        let destination = move_file_impl(&cfg, &src, true)?;
        assert_eq!(fs::read(destination)?, b"correct");
        assert!(!src.exists());
        assert_eq!(fs::read(stale)?, b"invalid");
        Ok(())
    }
}
