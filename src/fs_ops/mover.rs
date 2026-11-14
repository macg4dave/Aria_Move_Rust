//! One lifecycle for files and directory trees:
//!
//! 1. validate the source (type, links, disjoint from the destination);
//! 2. lock only this source download; revalidate under the lock;
//! 3. inventory the source, rejecting aria2 control files;
//! 4. publish by exclusive rename, or — across filesystems — copy into a
//!    staging directory, verify every file, publish that by exclusive rename,
//!    and only then remove the source, entry by entry.
//!
//! An existing destination is never replaced; a free name is chosen instead.
//! Once a copy is published, any later failure keeps the destination and any
//! source entries not yet removed.

use super::transfer::{self, Snapshot};
use super::{lock, metadata, naming, space};
use crate::{Config, errors, platform, shutdown};
use anyhow::{Context, Result, bail, ensure};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

/// One file or directory of the source. The root has an empty relative path.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    relative: PathBuf,
    directory: bool,
    snapshot: Snapshot,
}

/// Move a regular file or directory tree into `config.completed_base` and
/// return its final path. In a dry run nothing is created or modified, and
/// the returned path is the one that would be used now.
pub fn move_entry(config: &Config, src: &Path) -> Result<PathBuf> {
    move_with(config, src, false)
}

fn move_with(config: &Config, src: &Path, force_copy: bool) -> Result<PathBuf> {
    let src: &Path = &src.components().collect::<PathBuf>();
    validate_source(config, src)?;
    let name = src
        .file_name()
        .with_context(|| format!("source has no file name: {}", src.display()))?;
    let candidate = config.completed_base.join(name);
    if config.dry_run {
        inventory(src)?;
        return naming::unique_destination(&candidate)
            .map_err(|e| errors::io_error("select destination", &candidate, e));
    }
    platform::create_directory(&config.completed_base)
        .map_err(|e| errors::io_error("create completed_base", &config.completed_base, e))?;
    let _lock = (!config.disable_locks)
        .then(|| lock::acquire_source_lock(src))
        .transpose()?;
    validate_source(config, src)?;
    let entries = inventory(src)?;
    let mut dest = naming::unique_destination(&candidate)
        .map_err(|e| errors::io_error("select destination", &candidate, e))?;
    if !force_copy {
        loop {
            match rename(src, &dest) {
                Ok(true) => {
                    tracing::debug!(dest = %dest.display(), "moved by rename");
                    return Ok(dest);
                }
                Ok(false) => break,
                Err(error) if is_collision(&error) => {
                    dest = naming::unique_destination(&candidate)?
                }
                Err(error) => return Err(error),
            }
        }
    }
    tracing::debug!(dest = %dest.display(), "different filesystem; copying");
    copy_then_remove(config, src, &dest, &entries)
}

fn validate_source(config: &Config, src: &Path) -> Result<()> {
    shutdown::check()?;
    let meta = fs::symlink_metadata(src).map_err(|e| errors::source_error(src, e))?;
    if platform::is_link(&meta) {
        bail!("Refusing to move symlink: {}", src.display());
    }
    if !meta.is_file() && !meta.is_dir() {
        bail!(
            "Source path is neither a regular file nor a directory: {}",
            src.display()
        );
    }
    ensure!(
        !config.completed_base.as_os_str().is_empty(),
        "completed_base is not set"
    );
    platform::reject_symlinks(src)?;
    platform::reject_symlinks(&config.completed_base)?;
    let source = platform::normalize_missing(src)?;
    if !config.download_base.as_os_str().is_empty()
        && source == platform::normalize_missing(&config.download_base)?
    {
        bail!(
            "Refusing to move the download base itself: {}",
            src.display()
        );
    }
    let destination = platform::normalize_missing(&config.completed_base)?;
    if destination.starts_with(&source) || source.starts_with(&destination) {
        bail!(
            "Source '{}' and completed_base '{}' must not contain each other",
            source.display(),
            destination.display()
        );
    }
    Ok(())
}

/// `root` joined with `relative`, without a trailing separator for the root.
fn at(root: &Path, relative: &Path) -> PathBuf {
    if relative.as_os_str().is_empty() {
        root.to_path_buf()
    } else {
        root.join(relative)
    }
}

/// Every entry under `root` (root first, parents before children), rejecting
/// links, special files and in-progress names.
fn inventory(root: &Path) -> Result<Vec<Entry>> {
    // aria2 stores single-file progress beside the data, not inside it.
    let mut control = root.as_os_str().to_owned();
    control.push(".aria2");
    let control = PathBuf::from(control);
    match fs::symlink_metadata(&control) {
        Ok(_) => bail!("'{}' looks like an unfinished download", root.display()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(errors::io_error("inspect control file", &control, error)),
    }
    let mut entries = Vec::new();
    for item in walkdir::WalkDir::new(root)
        .follow_links(false)
        .sort_by_file_name()
    {
        shutdown::check()?;
        let item = item.with_context(|| format!("enumerate {}", root.display()))?;
        let path = item.path();
        let directory = item.file_type().is_dir();
        if !directory && !item.file_type().is_file() {
            bail!(
                "Unsupported entry (link or special file): {}",
                path.display()
            );
        }
        if !directory
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("aria2"))
        {
            bail!("'{}' looks like an unfinished download", path.display());
        }
        let handle = open_entry(path, directory)?;
        entries.push(Entry {
            relative: path.strip_prefix(root)?.to_path_buf(),
            directory,
            snapshot: Snapshot::of(&handle)?,
        });
    }
    Ok(entries)
}

fn open_entry(path: &Path, directory: bool) -> Result<File> {
    if directory {
        platform::open_directory(path)
    } else {
        platform::open_regular_read(path)
    }
    .map_err(|e| errors::io_error("open", path, e))
}

/// Recheck each entry immediately before removal: a whole-tree check can be
/// stale by the time a large cleanup reaches this entry. Never recurse.
fn remove_entry(source: &Path, entry: &Entry) -> Result<()> {
    let handle = open_entry(source, entry.directory)?;
    entry.snapshot.check(&handle, source)?;
    // Windows read handles deny deletion, so release after checking identity.
    drop(handle);
    if entry.directory {
        fs::remove_dir(source)
    } else {
        platform::remove_file(source)
    }
    .map_err(|e| errors::io_error("remove source", source, e))
}

/// Exclusive rename. `Ok(false)` means the destination is on another filesystem.
fn rename(src: &Path, dest: &Path) -> Result<bool> {
    shutdown::check()?;
    match platform::rename_noreplace(src, dest) {
        Ok(()) => {
            for dir in [platform::parent_dir(dest), platform::parent_dir(src)] {
                // The rename is committed; a failed sync cannot undo it.
                if let Err(error) = platform::sync_directory(dir) {
                    tracing::warn!(path = %dir.display(), %error, "directory sync failed after rename");
                }
            }
            Ok(true)
        }
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => Ok(false),
        Err(e) => Err(errors::io_error("rename into", dest, e)),
    }
}

fn is_collision(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<io::Error>()
        .is_some_and(|e| e.kind() == io::ErrorKind::AlreadyExists)
}

fn copy_then_remove(
    config: &Config,
    src: &Path,
    dest: &Path,
    entries: &[Entry],
) -> Result<PathBuf> {
    let (stage, staged, current) = stage_with_retries(src, entries, |current| {
        stage_copy(config, src, dest, current)
    })?;
    publish_copy(config, src, dest, &current, stage, staged)
}

fn stage_with_retries(
    src: &Path,
    entries: &[Entry],
    mut prepare: impl FnMut(&[Entry]) -> Result<(tempfile::TempDir, PathBuf)>,
) -> Result<(tempfile::TempDir, PathBuf, Vec<Entry>)> {
    let mut current = entries.to_vec();
    let mut attempt = 0;
    let (stage, staged) = loop {
        attempt += 1;
        match prepare(&current) {
            Ok(staged) => break staged,
            Err(error)
                if attempt < 3
                    && error
                        .downcast_ref::<transfer::SourceChanged>()
                        .is_some_and(|changed| changed.timestamp_only()) =>
            {
                tracing::warn!(source = %src.display(), attempt, %error,
                    "Source timestamp changed; retrying verified copy before publication");
                // Allow an attribute-cache refresh without an unbounded wait.
                // Staging was dropped before this wait; cancellation stays prompt.
                for _ in 0..40 {
                    shutdown::check()?;
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                let next = inventory(src)?;
                check_inventory_identity(src, entries, &next)?;
                current = next;
            }
            Err(error) => return Err(error),
        }
    };
    Ok((stage, staged, current))
}

/// A retry may refresh timestamps, but must not silently accept a replacement,
/// growth/shrinkage or a changed directory tree while waiting.
fn check_inventory_identity(src: &Path, expected: &[Entry], actual: &[Entry]) -> Result<()> {
    ensure!(
        actual.len() == expected.len(),
        "source entries changed before retry: {}",
        src.display()
    );
    for (expected, actual) in expected.iter().zip(actual) {
        ensure!(
            expected.relative == actual.relative && expected.directory == actual.directory,
            "source entries changed before retry: {}",
            src.display()
        );
        expected
            .snapshot
            .check_identity_and_size(&actual.snapshot, &at(src, &expected.relative))?;
    }
    Ok(())
}

fn stage_copy(
    config: &Config,
    src: &Path,
    dest: &Path,
    entries: &[Entry],
) -> Result<(tempfile::TempDir, PathBuf)> {
    let required = entries
        .iter()
        .try_fold(0u64, |sum, e| sum.checked_add(e.snapshot.len))
        .context("source size overflows u64")?;
    space::ensure_space_for_copy(&config.completed_base, required)?;

    // A tree is staged as the staging directory itself, so publication renames
    // within one parent; a file is staged inside it.
    let mut builder = tempfile::Builder::new();
    builder.prefix(".aria_move.stage.");
    #[cfg(unix)]
    {
        // Default (umask-filtered) mode rather than 0700: this may become the
        // published directory itself.
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o777));
    }
    let stage = builder
        .tempdir_in(&config.completed_base)
        .map_err(|e| errors::io_error("create staging directory in", &config.completed_base, e))?;
    let staged = if entries[0].directory {
        stage.path().to_path_buf()
    } else {
        stage.path().join(dest.file_name().unwrap_or_default())
    };

    for entry in entries
        .iter()
        .filter(|e| e.directory && !e.relative.as_os_str().is_empty())
    {
        shutdown::check()?;
        let target = staged.join(&entry.relative);
        fs::create_dir(&target).map_err(|e| errors::io_error("create directory", &target, e))?;
    }
    entries
        .iter()
        .filter(|entry| !entry.directory)
        .try_for_each(|entry| -> Result<()> {
            shutdown::check()?;
            let (source, target) = (at(src, &entry.relative), at(&staged, &entry.relative));
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|e| errors::io_error("create", &target, e))?;
            let (snapshot, meta) = transfer::copy_file(&source, &mut file)?;
            entry.snapshot.check_matches(
                &snapshot,
                &source,
                "source changed since it was inventoried",
            )?;
            drop(file);
            metadata::apply(config, &source, &target, &meta);
            Ok(())
        })?;
    check_inventory(src, entries, "source changed during copy")?;
    // Directory metadata last and deepest first, so children cannot disturb it.
    for entry in entries.iter().rev().filter(|e| e.directory) {
        let (source, target) = (at(src, &entry.relative), at(&staged, &entry.relative));
        metadata::apply(config, &source, &target, &fs::metadata(&source)?);
        platform::sync_directory(&target)?;
    }

    Ok((stage, staged))
}

fn check_inventory(src: &Path, expected: &[Entry], message: &str) -> Result<()> {
    let actual = inventory(src)?;
    ensure!(
        actual.len() == expected.len(),
        "{message}: {} (entry count changed)",
        src.display()
    );
    for (expected, actual) in expected.iter().zip(&actual) {
        ensure!(
            expected.relative == actual.relative && expected.directory == actual.directory,
            "{message}: {} (entries changed)",
            src.display()
        );
        expected
            .snapshot
            .check_matches(&actual.snapshot, &at(src, &expected.relative), message)?;
    }
    Ok(())
}

fn publish_copy(
    config: &Config,
    src: &Path,
    dest: &Path,
    entries: &[Entry],
    stage: tempfile::TempDir,
    staged: PathBuf,
) -> Result<PathBuf> {
    shutdown::check()?;
    let candidate = dest.to_path_buf();
    let mut dest = candidate.clone();
    loop {
        shutdown::check()?;
        match platform::rename_noreplace(&staged, &dest) {
            Ok(()) => break,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                dest = naming::unique_destination(&candidate)?;
            }
            Err(error) => return Err(errors::io_error("publish", &dest, error)),
        }
    }
    if entries[0].directory {
        // The staging directory was the published tree; its guard must not act.
        let _published = stage.keep();
    } else if let Err(error) = stage.close() {
        tracing::warn!(%error, "could not remove the empty staging directory");
    }
    platform::sync_directory(&config.completed_base)?;
    tracing::debug!(dest = %dest.display(), "verified copy published");

    // Published: failures keep the destination and remaining source entries.
    let retained = || {
        format!(
            "published {}; source {} retained",
            dest.display(),
            src.display()
        )
    };
    check_inventory(src, entries, "source changed after copying").with_context(retained)?;
    for entry in entries.iter().rev() {
        shutdown::check().with_context(retained)?;
        let source = at(src, &entry.relative);
        remove_entry(&source, entry).with_context(|| {
            format!(
                "published {}; cannot remove source {}",
                dest.display(),
                source.display()
            )
        })?;
    }
    platform::sync_directory(platform::parent_dir(src))?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        _temp: tempfile::TempDir,
        root: PathBuf,
        config: Config,
    }

    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let root = dunce::canonicalize(temp.path()).unwrap();
            let config = Config::new(root.join("incoming"), root.join("completed"));
            fs::create_dir(&config.download_base).unwrap();
            fs::create_dir(&config.completed_base).unwrap();
            Self {
                _temp: temp,
                root,
                config,
            }
        }

        fn incoming(&self, relative: &str) -> PathBuf {
            self.config.download_base.join(relative)
        }

        fn write(&self, relative: &str, bytes: &[u8]) -> PathBuf {
            let path = self.incoming(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, bytes).unwrap();
            path
        }

        fn completed_names(&self) -> Vec<String> {
            let mut names: Vec<_> = fs::read_dir(&self.config.completed_base)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }
    }

    /// Every behaviour below must hold for both strategies.
    #[test]
    fn a_destination_claimed_during_copy_is_retried_without_overwriting() -> Result<()> {
        let _guard = shutdown::test_guard();
        let fx = Fixture::new();
        let source = fx.write("same.bin", b"new payload");
        let candidate = fx.config.completed_base.join("same.bin");
        fs::write(&candidate, b"another completed download")?;
        let entries = inventory(&source)?;
        let actual = copy_then_remove(&fx.config, &source, &candidate, &entries)?;
        assert_ne!(actual, candidate);
        assert_eq!(fs::read(candidate)?, b"another completed download");
        assert_eq!(fs::read(actual)?, b"new payload");
        assert!(!source.exists());
        assert_eq!(fx.completed_names().len(), 2);
        Ok(())
    }

    #[test]
    fn duplicate_calls_for_one_source_publish_only_once() -> Result<()> {
        let _guard = shutdown::test_guard();
        for force_copy in [false, true] {
            let fx = Fixture::new();
            let source = fx.write("same.bin", b"payload");
            let barrier = std::sync::Barrier::new(2);
            let results = std::thread::scope(|scope| {
                let handles: Vec<_> = (0..2)
                    .map(|_| {
                        scope.spawn(|| {
                            barrier.wait();
                            move_with(&fx.config, &source, force_copy)
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .map(|h| h.join().unwrap())
                    .collect::<Vec<_>>()
            });
            assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
            assert_eq!(fx.completed_names(), ["same.bin"]);
            assert_eq!(
                fs::read(fx.config.completed_base.join("same.bin"))?,
                b"payload"
            );
            assert!(!source.exists());
        }
        Ok(())
    }

    const STRATEGIES: [bool; 2] = [false, true];

    #[test]
    fn concurrent_distinct_downloads_finish_for_both_strategies() -> Result<()> {
        let _guard = shutdown::test_guard();
        for force_copy in STRATEGIES {
            let fx = Fixture::new();
            let downloads: Vec<_> = (0..16)
                .map(|index| {
                    let bytes: Vec<_> = (0..(index * 131_073))
                        .map(|offset| ((offset + index) % 251) as u8)
                        .collect();
                    let name = format!("download {index}");
                    let tree = index % 2 == 1;
                    let source = if tree {
                        fx.write(&format!("{name}/nested/file.bin"), &bytes);
                        fx.write(&format!("{name}/second.txt"), b"second");
                        fx.incoming(&name)
                    } else {
                        fx.write(&name, &bytes)
                    };
                    (source, bytes, tree)
                })
                .collect();
            let barrier = std::sync::Barrier::new(downloads.len());
            std::thread::scope(|scope| -> Result<()> {
                let handles: Vec<_> = downloads
                    .iter()
                    .map(|(source, bytes, tree)| {
                        let barrier = &barrier;
                        let config = &fx.config;
                        scope.spawn(move || -> Result<()> {
                            barrier.wait();
                            let dest = move_with(config, source, force_copy)?;
                            let file = if *tree {
                                dest.join("nested/file.bin")
                            } else {
                                dest.clone()
                            };
                            assert_eq!(&fs::read(file)?, bytes);
                            if *tree {
                                assert_eq!(fs::read(dest.join("second.txt"))?, b"second");
                            }
                            assert!(!source.exists());
                            Ok(())
                        })
                    })
                    .collect();
                for handle in handles {
                    handle.join().unwrap()?;
                }
                Ok(())
            })?;
            assert_eq!(fx.completed_names().len(), downloads.len());
            assert_eq!(fs::read_dir(&fx.config.download_base)?.count(), 0);
        }
        Ok(())
    }

    #[test]
    fn files_move_with_identical_bytes() -> Result<()> {
        let _guard = shutdown::test_guard();
        for force_copy in STRATEGIES {
            let fx = Fixture::new();
            let bytes: Vec<u8> = (0..3_000_000u32).map(|n| (n % 253) as u8).collect();
            let src = fx.write("movie.mkv", &bytes);
            let dest = move_with(&fx.config, &src, force_copy)?;
            assert_eq!(dest, fx.config.completed_base.join("movie.mkv"));
            assert_eq!(fs::read(&dest)?, bytes);
            assert!(!src.exists());
            assert_eq!(
                fx.completed_names(),
                ["movie.mkv"],
                "no staging left (copy={force_copy})"
            );
        }
        Ok(())
    }

    #[test]
    fn trees_move_with_hidden_spaced_and_empty_entries() -> Result<()> {
        let _guard = shutdown::test_guard();
        for force_copy in STRATEGIES {
            let fx = Fixture::new();
            let files = [
                "a.txt",
                ".hidden/e.cfg",
                "sub 1/b.log",
                "sub 1/sub2/c.bin",
                "empty.bin",
            ];
            for file in files {
                fx.write(
                    &format!("tree/{file}"),
                    if file == "empty.bin" {
                        b""
                    } else {
                        file.as_bytes()
                    },
                );
            }
            fs::create_dir_all(fx.incoming("tree/empty dir/nested"))?;
            let src = fx.incoming("tree");
            let dest = move_with(&fx.config, &src, force_copy)?;
            assert!(!src.exists());
            for file in files {
                let expected: &[u8] = if file == "empty.bin" {
                    b""
                } else {
                    file.as_bytes()
                };
                assert_eq!(
                    fs::read(dest.join(file))?,
                    expected,
                    "{file} (copy={force_copy})"
                );
            }
            assert!(dest.join("empty dir/nested").is_dir());
            assert_eq!(fx.completed_names(), ["tree"]);
        }
        Ok(())
    }

    #[test]
    fn existing_destinations_are_never_replaced() -> Result<()> {
        let _guard = shutdown::test_guard();
        for force_copy in STRATEGIES {
            let fx = Fixture::new();
            let existing = fx.config.completed_base.join("item");
            fs::create_dir(&existing)?;
            fs::write(existing.join("keep"), b"keep")?;
            fx.write("item/new", b"new");
            let dest = move_with(&fx.config, &fx.incoming("item"), force_copy)?;
            assert_ne!(dest, existing);
            assert_eq!(fs::read(existing.join("keep"))?, b"keep");
            assert_eq!(fs::read(dest.join("new"))?, b"new");
            let file = fx.write("item.txt", b"second");
            fs::write(fx.config.completed_base.join("item.txt"), b"first")?;
            let dest = move_with(&fx.config, &file, force_copy)?;
            assert_eq!(
                fs::read(fx.config.completed_base.join("item.txt"))?,
                b"first"
            );
            assert_eq!(fs::read(&dest)?, b"second");
        }
        Ok(())
    }

    #[test]
    fn dry_run_changes_nothing_even_when_completed_base_is_missing() -> Result<()> {
        let _guard = shutdown::test_guard();
        let mut fx = Fixture::new();
        fs::remove_dir(&fx.config.completed_base)?;
        fx.config.completed_base = fx.root.join("missing").join("completed");
        fx.config.dry_run = true;
        let file = fx.write("file", b"x");
        fs::create_dir_all(fx.incoming("dir/empty"))?;
        let before = walkdir::WalkDir::new(&fx.root).into_iter().count();
        assert_eq!(
            move_entry(&fx.config, &file)?,
            fx.config.completed_base.join("file")
        );
        assert_eq!(
            move_entry(&fx.config, &fx.incoming("dir"))?,
            fx.config.completed_base.join("dir")
        );
        assert_eq!(walkdir::WalkDir::new(&fx.root).into_iter().count(), before);
        assert!(file.exists());
        Ok(())
    }

    #[test]
    fn unfinished_downloads_are_refused_untouched() -> Result<()> {
        let _guard = shutdown::test_guard();
        for force_copy in STRATEGIES {
            let fx = Fixture::new();
            for name in ["single.aria2", "tree/done.bin", "tree/next.ARIA2"] {
                fx.write(name, b"partial");
            }
            for name in ["single.aria2", "tree"] {
                let error = move_with(&fx.config, &fx.incoming(name), force_copy).unwrap_err();
                assert!(error.to_string().contains("unfinished download"), "{error}");
            }
            assert!(fx.incoming("tree/done.bin").exists());
            assert!(fx.completed_names().is_empty());
        }
        Ok(())
    }

    #[test]
    fn completed_files_with_temporary_looking_extensions_are_valid() -> Result<()> {
        let _guard = shutdown::test_guard();
        for force_copy in STRATEGIES {
            let fx = Fixture::new();
            for name in ["data.tmp", "model.part", "archive.crdownload"] {
                let source = fx.write(name, b"complete");
                let destination = move_with(&fx.config, &source, force_copy)?;
                assert_eq!(fs::read(destination)?, b"complete");
                assert!(!source.exists());
            }
        }
        Ok(())
    }

    #[test]
    fn adjacent_aria2_control_files_block_files_and_trees() -> Result<()> {
        let _guard = shutdown::test_guard();
        for force_copy in STRATEGIES {
            for dry_run in [false, true] {
                let mut fx = Fixture::new();
                fx.config.dry_run = dry_run;
                fx.write("file.bin", b"partial");
                fx.write("tree/child", b"partial");
                for name in ["file.bin", "tree"] {
                    let control = fx.write(&format!("{name}.aria2"), b"progress");
                    let error = move_with(&fx.config, &fx.incoming(name), force_copy).unwrap_err();
                    assert!(
                        error.to_string().contains("unfinished download"),
                        "{error:#}"
                    );
                    assert!(fx.incoming(name).exists());
                    assert_eq!(fs::read(control)?, b"progress");
                }
                assert!(fx.completed_names().is_empty());
            }
        }
        Ok(())
    }

    #[test]
    fn invalid_sources_are_rejected() -> Result<()> {
        let _guard = shutdown::test_guard();
        let fx = Fixture::new();
        let cases = [
            (fx.config.download_base.clone(), "download base itself"),
            (fx.incoming("missing"), "Source path not found"),
            (
                fx.config.completed_base.clone(),
                "must not contain each other",
            ),
        ];
        for (src, message) in cases {
            let error = move_entry(&fx.config, &src).unwrap_err();
            assert!(error.to_string().contains(message), "{src:?}: {error}");
        }
        let mut nested = fx.config.clone();
        nested.completed_base = fx.incoming("tree/inside");
        fs::create_dir_all(&nested.completed_base)?;
        assert!(move_entry(&nested, &fx.incoming("tree")).is_err());
        let unset = Config::default();
        let error = move_entry(&unset, &fx.write("file", b"x")).unwrap_err();
        assert!(
            error.to_string().contains("completed_base is not set"),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn interruption_before_publication_leaves_no_trace() -> Result<()> {
        let _guard = shutdown::test_guard();
        let fx = Fixture::new();
        let src = fx.write("tree/file", b"data");
        shutdown::request();
        let error = move_with(&fx.config, &fx.incoming("tree"), true).unwrap_err();
        assert_eq!(errors::error_code(&error), "interrupted");
        assert_eq!(fs::read(src)?, b"data");
        assert!(fx.completed_names().is_empty());
        Ok(())
    }

    #[test]
    fn cleanup_refuses_changed_or_replaced_entries() -> Result<()> {
        let _guard = shutdown::test_guard();
        let fx = Fixture::new();
        let source = fx.write("file", b"original");
        let entries = inventory(&source)?;
        fs::write(&source, b"changed contents")?;
        assert!(remove_entry(&source, &entries[0]).is_err());
        assert_eq!(fs::read(&source)?, b"changed contents");

        let entries = inventory(&source)?;
        let replacement = fx.write("replacement", b"changed contents");
        fs::remove_file(&source)?;
        fs::rename(replacement, &source)?;
        assert!(remove_entry(&source, &entries[0]).is_err());
        assert_eq!(fs::read(&source)?, b"changed contents");

        fs::create_dir(fx.incoming("directory"))?;
        let directory = fx.incoming("directory");
        let entries = inventory(&directory)?;
        fs::write(directory.join("unexpected"), b"keep")?;
        assert!(remove_entry(&directory, &entries[0]).is_err());
        assert_eq!(fs::read(directory.join("unexpected"))?, b"keep");
        Ok(())
    }

    #[test]
    fn copy_stage_failure_keeps_source_and_cleans_staging() -> Result<()> {
        let _guard = shutdown::test_guard();
        let fx = Fixture::new();
        let src = fx.write("tree/file", b"data");
        let entries = inventory(&fx.incoming("tree"))?;
        fs::write(&src, b"changed!")?;
        let dest = fx.config.completed_base.join("tree");
        let error =
            copy_then_remove(&fx.config, &fx.incoming("tree"), &dest, &entries).unwrap_err();
        assert!(error.to_string().contains("changed"), "{error}");
        assert_eq!(fs::read(&src)?, b"changed!");
        assert!(fx.completed_names().is_empty());
        Ok(())
    }

    #[test]
    fn timestamp_changes_retry_a_fresh_verified_copy_before_publication() -> Result<()> {
        let _guard = shutdown::test_guard();
        for timestamp_only in [false, true] {
            let fx = Fixture::new();
            let source = fx.write("file.bin", b"original");
            filetime::set_file_mtime(
                &source,
                filetime::FileTime::from_unix_time(1_700_000_000, 0),
            )?;
            let entries = inventory(&source)?;
            if !timestamp_only {
                fs::write(&source, b"modified")?;
            }
            filetime::set_file_mtime(
                &source,
                filetime::FileTime::from_unix_time(1_700_000_001, 0),
            )?;
            let dest = fx.config.completed_base.join("file.bin");
            let error = stage_copy(&fx.config, &source, &dest, &entries).unwrap_err();
            let message = error.to_string();
            assert!(
                message.contains("source changed since it was inventoried"),
                "{message}"
            );
            assert!(message.contains("size: 8 -> 8"), "{message}");
            assert!(message.contains("modified:"), "{message}");
            let expected = if timestamp_only {
                b"original"
            } else {
                b"modified"
            };
            assert_eq!(fs::read(&source)?, expected);
            assert!(fx.completed_names().is_empty());
            // The same hook refreshes its inventory and recopies verified bytes.
            let dest = copy_then_remove(&fx.config, &source, &dest, &entries)?;
            assert_eq!(fs::read(dest)?, expected);
            assert!(!source.exists());
        }
        Ok(())
    }

    #[test]
    fn continuously_changing_timestamps_exhaust_three_attempts_without_publication() -> Result<()> {
        let _guard = shutdown::test_guard();
        let fx = Fixture::new();
        let source = fx.write("file.bin", b"unchanged");
        let dest = fx.config.completed_base.join("file.bin");
        let entries = inventory(&source)?;
        let mut attempts = 0;
        let error = stage_with_retries(&source, &entries, |current| {
            attempts += 1;
            filetime::set_file_mtime(
                &source,
                filetime::FileTime::from_unix_time(1_700_000_000 + attempts, 0),
            )?;
            stage_copy(&fx.config, &source, &dest, current)
        })
        .unwrap_err();
        assert_eq!(attempts, 3);
        assert!(
            error
                .downcast_ref::<transfer::SourceChanged>()
                .unwrap()
                .timestamp_only()
        );
        assert_eq!(fs::read(source)?, b"unchanged");
        assert!(fx.completed_names().is_empty());
        Ok(())
    }

    #[test]
    fn timestamp_update_after_copy_retries_and_cancellation_interrupts_retry_wait() -> Result<()> {
        let _guard = shutdown::test_guard();
        for cancel in [false, true] {
            let fx = Fixture::new();
            let source = fx.write("file.bin", b"complete payload");
            let dest = fx.config.completed_base.join("file.bin");
            filetime::set_file_mtime(
                &source,
                filetime::FileTime::from_unix_time(1_700_000_000, 0),
            )?;
            let entries = inventory(&source)?;
            let mut attempts = 0;
            let result = stage_with_retries(&source, &entries, |current| {
                attempts += 1;
                let staged = stage_copy(&fx.config, &source, &dest, current)?;
                if attempts == 1 {
                    filetime::set_file_mtime(
                        &source,
                        filetime::FileTime::from_unix_time(1_700_000_001, 0),
                    )?;
                    if cancel {
                        shutdown::request();
                    }
                }
                current[0]
                    .snapshot
                    .check(&open_entry(&source, false)?, &source)?;
                Ok(staged)
            });
            if cancel {
                assert_eq!(errors::error_code(&result.unwrap_err()), "interrupted");
                assert_eq!(attempts, 1);
                assert!(fx.completed_names().is_empty());
                assert_eq!(fs::read(source)?, b"complete payload");
            } else {
                let (stage, staged, current) = result?;
                assert_eq!(attempts, 2);
                // Only the successful attempt's staging directory survives.
                assert_eq!(fx.completed_names().len(), 1);
                publish_copy(&fx.config, &source, &dest, &current, stage, staged)?;
                assert_eq!(fs::read(dest)?, b"complete payload");
                assert!(!source.exists());
            }
        }
        Ok(())
    }

    #[test]
    fn retries_refuse_replacements_size_changes_and_changed_tree_entries() -> Result<()> {
        let _guard = shutdown::test_guard();
        for change in ["replace", "resize", "add", "remove"] {
            let fx = Fixture::new();
            let file = fx.write("tree/file.bin", b"original");
            let source = fx.incoming("tree");
            let entries = inventory(&source)?;
            match change {
                "replace" => {
                    let replacement = fx.write("replacement", b"original");
                    fs::remove_file(&file)?;
                    fs::rename(replacement, &file)?;
                }
                "resize" => fs::write(&file, b"longer contents")?,
                "add" => {
                    fx.write("tree/extra.bin", b"extra");
                }
                "remove" => fs::remove_file(&file)?,
                _ => unreachable!(),
            }
            assert!(
                check_inventory_identity(&source, &entries, &inventory(&source)?).is_err(),
                "{change}"
            );
            assert!(source.exists());
            assert!(fx.completed_names().is_empty());
        }
        Ok(())
    }

    #[test]
    fn timestamp_changes_after_publication_retain_source_without_retrying() -> Result<()> {
        let _guard = shutdown::test_guard();
        let fx = Fixture::new();
        let source = fx.write("file.bin", b"verified");
        let dest = fx.config.completed_base.join("file.bin");
        filetime::set_file_mtime(
            &source,
            filetime::FileTime::from_unix_time(1_700_000_000, 0),
        )?;
        let entries = inventory(&source)?;
        let (stage, staged) = stage_copy(&fx.config, &source, &dest, &entries)?;
        filetime::set_file_mtime(
            &source,
            filetime::FileTime::from_unix_time(1_700_000_001, 0),
        )?;
        let error = publish_copy(&fx.config, &source, &dest, &entries, stage, staged).unwrap_err();
        assert!(format!("{error:#}").contains("retained"));
        assert_eq!(fs::read(source)?, b"verified");
        assert_eq!(fs::read(dest)?, b"verified");
        assert_eq!(fx.completed_names(), ["file.bin"]);
        Ok(())
    }

    #[test]
    fn copy_preserves_metadata_when_configured() -> Result<()> {
        let _guard = shutdown::test_guard();
        let mut fx = Fixture::new();
        fx.config.preserve_metadata = true;
        let file = fx.write("tree/file", b"x");
        let modified = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        filetime::set_file_mtime(&file, modified)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&file, fs::Permissions::from_mode(0o640))?;
            fs::set_permissions(fx.incoming("tree"), fs::Permissions::from_mode(0o750))?;
        }
        let dest = move_with(&fx.config, &fx.incoming("tree"), true)?;
        let meta = fs::metadata(dest.join("file"))?;
        assert_eq!(
            filetime::FileTime::from_last_modification_time(&meta),
            modified
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(meta.permissions().mode() & 0o777, 0o640);
            assert_eq!(fs::metadata(&dest)?.permissions().mode() & 0o777, 0o750);
        }
        Ok(())
    }

    #[test]
    fn read_only_sources_are_removed_after_a_copy() -> Result<()> {
        let _guard = shutdown::test_guard();
        let fx = Fixture::new();
        let file = fx.write("ro.bin", b"x");
        let mut permissions = fs::metadata(&file)?.permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&file, permissions)?;
        let dest = move_with(&fx.config, &file, true)?;
        assert!(!file.exists());
        assert_eq!(fs::read(&dest)?, b"x");
        platform::remove_file(&dest)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn links_and_special_entries_inside_trees_are_refused() -> Result<()> {
        let _guard = shutdown::test_guard();
        let fx = Fixture::new();
        fx.write("tree/ordinary", b"keep");
        std::os::unix::fs::symlink(fx.root.join("missing"), fx.incoming("tree/link"))?;
        assert!(move_entry(&fx.config, &fx.incoming("tree")).is_err());
        fs::remove_file(fx.incoming("tree/link"))?;
        let fifo = fx.incoming("tree/fifo");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()?
                .success()
        );
        assert!(move_entry(&fx.config, &fx.incoming("tree")).is_err());
        assert_eq!(fs::read(fx.incoming("tree/ordinary"))?, b"keep");
        assert!(fx.completed_names().is_empty());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_entries_fail_without_changes() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let _guard = shutdown::test_guard();
        // SAFETY: geteuid has no preconditions. Root bypasses permission bits.
        if unsafe { libc::geteuid() } == 0 {
            return Ok(());
        }
        let fx = Fixture::new();
        fx.write("tree/private/keep", b"keep");
        let private = fx.incoming("tree/private");
        fs::set_permissions(&private, fs::Permissions::from_mode(0o000))?;
        let result = move_with(&fx.config, &fx.incoming("tree"), true);
        fs::set_permissions(&private, fs::Permissions::from_mode(0o700))?;
        assert!(result.is_err());
        assert_eq!(fs::read(private.join("keep"))?, b"keep");
        assert!(fx.completed_names().is_empty());
        Ok(())
    }
}
