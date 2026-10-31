//! Directory lifecycle. A fallback is published only after a complete verified tree.
use super::atomic::{MoveOutcome, try_atomic_move};
use super::copy::Snapshot;
use crate::{Config, shutdown};
use anyhow::{Context, Result, ensure};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
struct Entry {
    relative: PathBuf,
    directory: bool,
    snapshot: Snapshot,
}

fn inventory(root: &Path) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        shutdown::check()?;
        let entry = entry.with_context(|| format!("enumerate {}", root.display()))?;
        crate::config::paths::reject_symlinks(entry.path())?;
        let meta = entry.metadata()?;
        ensure!(
            meta.is_dir() || meta.is_file(),
            "unsupported tree entry: {}",
            entry.path().display()
        );
        let handle = if meta.is_file() {
            crate::platform::open_regular_read(entry.path())?
        } else {
            crate::platform::open_directory(entry.path())?
        };
        entries.push(Entry {
            relative: entry.path().strip_prefix(root)?.to_path_buf(),
            directory: meta.is_dir(),
            snapshot: Snapshot::read(&handle)?,
        });
    }
    entries.sort_by(|a, b| a.relative.cmp(&b.relative));
    Ok(entries)
}

/// Move a directory, preserving empty directories and rejecting unsupported entries.
pub fn move_dir(config: &Config, src: &Path) -> Result<PathBuf> {
    move_dir_impl(config, src, false)
}

fn move_dir_impl(config: &Config, src: &Path, force_copy: bool) -> Result<PathBuf> {
    super::entry::validate_source(config, src, true)?;
    let candidate = config
        .completed_base
        .join(src.file_name().context("source directory has no name")?);
    if config.dry_run {
        inventory(src)?;
        return Ok(super::duplicate::unique_destination(&candidate));
    }
    crate::config::paths::create_directory(&config.completed_base)?;
    let _locks =
        super::lock::acquire_move_locks(src, &config.completed_base, config.disable_locks)?;
    super::entry::validate_source(config, src, true)?;
    let entries = inventory(src)?;
    let dest = super::duplicate::unique_destination(&candidate);
    if !force_copy && try_atomic_move(src, &dest)? == MoveOutcome::Renamed {
        return Ok(dest);
    }
    copy_tree(config, src, &dest, &entries)
}

fn copy_tree(config: &Config, src: &Path, dest: &Path, entries: &[Entry]) -> Result<PathBuf> {
    let mut required = 0u64;
    for entry in entries.iter().filter(|e| !e.directory) {
        required = required
            .checked_add(fs::metadata(src.join(&entry.relative))?.len())
            .context("tree size overflow")?;
    }
    super::space::ensure_space_for_copy(&config.completed_base, required)?;
    let stage = tempfile::Builder::new()
        .prefix(".aria_move.tree.")
        .tempdir_in(&config.completed_base)?;
    for entry in entries.iter().filter(|e| e.directory) {
        shutdown::check()?;
        crate::config::paths::create_directory(&stage.path().join(&entry.relative))?;
    }
    entries
        .par_iter()
        .filter(|entry| !entry.directory)
        .try_for_each(|entry| -> Result<()> {
            shutdown::check()?;
            let source = src.join(&entry.relative);
            ensure!(
                !crate::utils::file_is_mutable(&source)?,
                "File '{}' seems in-use",
                source.display()
            );
            let target = stage.path().join(&entry.relative);
            let mut file = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&target)?;
            let (snapshot, _) = super::copy::copy_to_file(
                &source,
                &mut file,
                config.preserve_metadata,
                config.preserve_permissions,
                &target,
            )?;
            ensure!(
                snapshot == entry.snapshot,
                "source changed since tree enumeration"
            );
            Ok(())
        })?;
    let after = inventory(src)?;
    ensure!(after == entries, "source tree changed during copy");
    // Restore directory metadata after children, deepest first, while still owned.
    for entry in entries.iter().rev().filter(|e| e.directory) {
        let source = src.join(&entry.relative);
        let target = stage.path().join(&entry.relative);
        super::copy::apply_metadata(
            &source,
            &target,
            &fs::metadata(&source)?,
            config.preserve_metadata,
            config.preserve_permissions,
        )?;
        crate::platform::sync_directory(&target)?;
    }
    shutdown::check()?;
    crate::platform::rename_noreplace(stage.path(), dest)?;
    // The old staging path is now absent. Its guard never owns the published name.
    crate::platform::sync_directory(&config.completed_base)?;
    ensure!(
        inventory(src)? == entries,
        "destination published but source tree changed; retaining source"
    );
    // Verify ALL files before deleting ANY. Never recursively delete a source tree:
    // unexpected new entries must survive, and remove_dir must fail when nonempty.
    for entry in entries.iter().filter(|e| !e.directory) {
        super::file_move::verify_source(
            &src.join(&entry.relative),
            &dest.join(&entry.relative),
            &entry.snapshot,
        )?;
    }
    for entry in entries.iter().rev().filter(|e| !e.directory) {
        shutdown::check()?;
        let source = src.join(&entry.relative);
        entry
            .snapshot
            .check(&crate::platform::open_regular_read(&source)?)?;
        fs::remove_file(&source).with_context(|| {
            format!(
                "destination published at {}; remove source {}",
                dest.display(),
                source.display()
            )
        })?;
    }
    for entry in entries.iter().rev().filter(|e| e.directory) {
        shutdown::check()?;
        fs::remove_dir(src.join(&entry.relative))
            .context("destination published; source directory is not empty or cannot be removed")?;
    }
    if let Some(parent) = src.parent() {
        crate::platform::sync_directory(parent)?;
    }
    Ok(dest.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forced_copy_preserves_nested_hidden_empty_and_bytes() -> Result<()> {
        let _guard = crate::shutdown::test_guard();
        let td = tempfile::tempdir()?;
        let incoming = td.path().join("in");
        let completed = td.path().join("out");
        let src = incoming.join("tree");
        fs::create_dir_all(src.join(".hidden/sub dir/empty"))?;
        fs::create_dir_all(&completed)?;
        let cfg = Config::new(&incoming, &completed);
        fs::write(src.join(".hidden/sub dir/data"), b"bytes")?;
        fs::write(src.join("root"), b"root")?;
        let dest = move_dir_impl(&cfg, &src, true)?;
        assert!(!src.exists());
        assert_eq!(fs::read(dest.join(".hidden/sub dir/data"))?, b"bytes");
        assert!(dest.join(".hidden/sub dir/empty").is_dir());
        Ok(())
    }

    #[test]
    fn fallback_failure_keeps_source_and_existing_destination() -> Result<()> {
        let _guard = crate::shutdown::test_guard();
        let td = tempfile::tempdir()?;
        let src = td.path().join("in/tree");
        let completed = td.path().join("out");
        fs::create_dir_all(&src)?;
        fs::create_dir_all(&completed)?;
        fs::write(src.join("unfinished.part"), b"source")?;
        let cfg = Config::new(src.parent().unwrap(), &completed);
        let target = completed.join("tree");
        fs::create_dir(&target)?;
        fs::write(target.join("unrelated"), b"keep")?;
        assert!(move_dir_impl(&cfg, &src, true).is_err());
        assert_eq!(fs::read(src.join("unfinished.part"))?, b"source");
        assert_eq!(fs::read(target.join("unrelated"))?, b"keep");
        assert_eq!(fs::read_dir(&completed)?.count(), 1);
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn unreadable_tree_is_an_error_and_source_is_preserved() -> Result<()> {
        let _guard = crate::shutdown::test_guard();
        use std::os::unix::fs::PermissionsExt;
        // SAFETY: geteuid has no preconditions and does not change process state.
        if unsafe { libc::geteuid() } == 0 {
            return Ok(());
        }
        let td = tempfile::tempdir()?;
        let root = dunce::canonicalize(td.path())?;
        let src = root.join("incoming/tree");
        let inaccessible = src.join("private");
        let completed = root.join("out");
        fs::create_dir_all(&inaccessible)?;
        fs::create_dir(&completed)?;
        fs::write(inaccessible.join("keep"), b"keep")?;
        fs::set_permissions(&inaccessible, fs::Permissions::from_mode(0o000))?;
        let cfg = Config::new(src.parent().unwrap(), &completed);
        let result = move_dir_impl(&cfg, &src, true);
        fs::set_permissions(&inaccessible, fs::Permissions::from_mode(0o700))?;
        assert!(result.is_err());
        assert_eq!(fs::read(inaccessible.join("keep"))?, b"keep");
        assert!(fs::read_dir(completed)?.next().is_none());
        Ok(())
    }
}
