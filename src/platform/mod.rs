//! Filesystem primitives with one shared policy.
//!
//! Everything here is platform-independent except the small native layer in
//! `unix.rs` / `windows.rs` (exclusive rename, free space, file identity, link
//! detection, open flags, source locks and sync).
//!
//! Policy: symlinks and Windows reparse points are refused anywhere along a
//! path, and publication never replaces an existing destination.

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as sys;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as sys;

pub(crate) use sys::{file_identity, free_space, is_link, sync_directory, try_lock_source};

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

/// The parent directory of `path`, or `.` for a bare name.
pub(crate) fn parent_dir(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

/// Whether an existing ancestor of `path` is a symlink or reparse point.
/// Missing ancestors are ignored; other metadata errors are returned.
pub(crate) fn path_has_symlink_ancestor(path: &Path) -> io::Result<bool> {
    for ancestor in path.ancestors().skip(1) {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        match fs::symlink_metadata(ancestor) {
            Ok(meta) if is_link(&meta) => return Ok(true),
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(false)
}

/// Fail if `path` or any existing ancestor is a symlink or reparse point.
pub(crate) fn reject_symlinks(path: &Path) -> io::Result<()> {
    let leaf_is_link = match fs::symlink_metadata(path) {
        Ok(meta) => is_link(&meta),
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => return Err(e),
    };
    if leaf_is_link || path_has_symlink_ancestor(path)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Refusing symlink or reparse point: {}", path.display()),
        ));
    }
    Ok(())
}

/// Absolute, lexically normalized form of a possibly missing path, with its
/// longest existing prefix canonicalized. Never writes.
pub(crate) fn normalize_missing(path: &Path) -> io::Result<PathBuf> {
    let mut normalized = PathBuf::new();
    for part in std::path::absolute(path)?.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(part),
        }
    }
    let mut existing = normalized.as_path();
    let mut missing = Vec::new();
    while !existing.try_exists()? {
        let (Some(name), Some(parent)) = (existing.file_name(), existing.parent()) else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no existing ancestor: {}", path.display()),
            ));
        };
        missing.push(name);
        existing = parent;
    }
    let mut result = dunce::canonicalize(existing)?;
    result.extend(missing.into_iter().rev());
    Ok(result)
}

/// Create missing directories (owner-only on Unix). Existing directories are
/// accepted as they are; their permissions are never changed.
pub(crate) fn create_directory(path: &Path) -> io::Result<()> {
    reject_symlinks(path)?;
    if path.is_dir() {
        return Ok(());
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        create_directory(parent)?;
    }
    let mut builder = DirBuilder::new();
    sys::configure_private_dir(&mut builder);
    match builder.create(path) {
        Err(e) if !(e.kind() == io::ErrorKind::AlreadyExists && path.is_dir()) => return Err(e),
        _ => {}
    }
    reject_symlinks(path)
}

/// Rename without ever replacing an existing destination (including a
/// dangling link). Cross-device moves fail with `ErrorKind::CrossesDevices`.
pub(crate) fn rename_noreplace(src: &Path, dst: &Path) -> io::Result<()> {
    reject_symlinks(src)?;
    reject_symlinks(parent_dir(dst))?;
    sys::rename_noreplace(src, dst)
}

fn open_checked(path: &Path, options: &OpenOptions, directory: bool) -> io::Result<File> {
    reject_symlinks(path)?;
    let file = options.open(path)?;
    let meta = file.metadata()?;
    let expected = if directory {
        meta.is_dir()
    } else {
        meta.is_file()
    };
    if !expected || is_link(&meta) {
        return Err(io::Error::new(
            if directory {
                io::ErrorKind::NotADirectory
            } else {
                io::ErrorKind::InvalidInput
            },
            format!(
                "expected a regular {}: {}",
                if directory { "directory" } else { "file" },
                path.display()
            ),
        ));
    }
    Ok(file)
}

/// Open a regular file for reading without following a final link. On Windows
/// the handle also denies concurrent writers and deletion while it is held.
pub(crate) fn open_regular_read(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    sys::configure_read(&mut options);
    open_checked(path, &options, false)
}

/// Open a directory handle (for identity and sync) without following links.
pub(crate) fn open_directory(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    sys::configure_directory(&mut options);
    open_checked(path, &options, true)
}

/// Open a log for appending, creating it (0600 on Unix) and its directories.
/// Existing files keep their permissions.
pub(crate) fn open_log_append(path: &Path) -> io::Result<File> {
    create_directory(parent_dir(path))?;
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    sys::configure_log(&mut options);
    open_checked(path, &options, false)
}

/// Durably write a new file (0600 on Unix). Fails if `path` already exists.
pub(crate) fn write_new_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    reject_symlinks(path)?;
    let parent = parent_dir(path);
    create_directory(parent)?;
    let mut stage = tempfile::Builder::new()
        .prefix(".aria_move.config.")
        .tempfile_in(parent)?;
    stage.write_all(contents)?;
    stage.as_file().sync_all()?;
    rename_noreplace(stage.path(), path)?;
    // The temp guard now refers to a vanished name; dropping it removes nothing.
    sync_directory(parent)
}

/// Remove a file, clearing a Windows read-only attribute that would block it.
pub(crate) fn remove_file(path: &Path) -> io::Result<()> {
    sys::prepare_removal(path)?;
    fs::remove_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(temp.path()).unwrap();
        (temp, root)
    }

    fn file_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn rename_noreplace_moves_and_refuses_to_replace() {
        let (_temp, root) = root();
        let (src, dst) = (root.join("src"), root.join("dst"));
        fs::write(&src, b"new").unwrap();
        fs::write(&dst, b"old").unwrap();
        let error = rename_noreplace(&src, &dst).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&dst).unwrap(), b"old");
        fs::remove_file(&dst).unwrap();
        rename_noreplace(&src, &dst).unwrap();
        assert_eq!(fs::read(&dst).unwrap(), b"new");
        assert!(!src.exists());
    }

    #[test]
    fn rename_noreplace_moves_directories_and_refuses_nonempty_targets() {
        let (_temp, root) = root();
        let (src, dst) = (root.join("tree"), root.join("published"));
        fs::create_dir_all(src.join("nested")).unwrap();
        fs::create_dir(&dst).unwrap();
        assert!(rename_noreplace(&src, &dst).is_err());
        fs::remove_dir(&dst).unwrap();
        rename_noreplace(&src, &dst).unwrap();
        assert!(dst.join("nested").is_dir());
    }

    #[test]
    fn normalize_missing_resolves_dots_against_existing_prefix() {
        let (_temp, root) = root();
        fs::create_dir(root.join("a")).unwrap();
        let input = root
            .join("a")
            .join(".")
            .join("b")
            .join("..")
            .join("c")
            .join("d");
        assert_eq!(
            normalize_missing(&input).unwrap(),
            root.join("a").join("c").join("d")
        );
        assert_eq!(normalize_missing(&root).unwrap(), root);
    }

    #[test]
    fn create_directory_is_recursive_and_idempotent() {
        let (_temp, root) = root();
        let nested = root.join("x").join("y").join("z");
        create_directory(&nested).unwrap();
        create_directory(&nested).unwrap();
        assert!(nested.is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&nested).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700);
        }
    }

    #[test]
    fn create_directory_rejects_existing_file() {
        let (_temp, root) = root();
        let file = root.join("file");
        fs::write(&file, b"x").unwrap();
        assert!(create_directory(&file).is_err());
    }

    #[test]
    fn open_regular_read_rejects_directories_and_missing_paths() {
        let (_temp, root) = root();
        assert!(open_regular_read(&root).is_err());
        assert_eq!(
            open_regular_read(&root.join("missing")).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert!(open_directory(&root).is_ok());
        let file = root.join("f");
        fs::write(&file, b"x").unwrap();
        assert!(open_directory(&file).is_err());
    }

    #[test]
    fn write_new_file_never_replaces_and_leaves_no_temporaries() {
        let (_temp, root) = root();
        let path = root.join("sub").join("config.xml");
        write_new_file(&path, b"original").unwrap();
        assert!(write_new_file(&path, b"replacement").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert_eq!(file_names(path.parent().unwrap()), ["config.xml"]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn write_new_file_onto_a_directory_fails_cleanly() {
        let (_temp, root) = root();
        let path = root.join("config.xml");
        fs::create_dir(&path).unwrap();
        assert!(write_new_file(&path, b"x").is_err());
        assert_eq!(file_names(&root), ["config.xml"]);
    }

    #[test]
    fn log_append_creates_parents_and_appends() {
        let (_temp, root) = root();
        let path = root.join("logs").join("aria_move.log");
        open_log_append(&path).unwrap().write_all(b"one\n").unwrap();
        open_log_append(&path).unwrap().write_all(b"two\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "one\ntwo\n");
        assert!(open_log_append(&root).is_err(), "a directory is not a log");
    }

    #[cfg(unix)]
    #[test]
    fn log_mode_is_0600_when_created_and_kept_when_existing() {
        use std::os::unix::fs::PermissionsExt;
        let (_temp, root) = root();
        let created = root.join("new.log");
        open_log_append(&created).unwrap();
        assert_eq!(
            fs::metadata(&created).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let existing = root.join("old.log");
        fs::write(&existing, b"").unwrap();
        fs::set_permissions(&existing, fs::Permissions::from_mode(0o640)).unwrap();
        open_log_append(&existing).unwrap();
        assert_eq!(
            fs::metadata(&existing).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }

    #[test]
    fn free_space_reports_a_directory_and_rejects_embedded_nul() {
        let (_temp, root) = root();
        assert!(free_space(&root).unwrap() > 0);
        let error = free_space(Path::new(".\0truncated")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn file_identity_distinguishes_files_and_survives_reopen() {
        let (_temp, root) = root();
        let (a, b) = (root.join("a"), root.join("b"));
        fs::write(&a, b"a").unwrap();
        fs::write(&b, b"b").unwrap();
        let id = |p: &Path| {
            let file = File::open(p).unwrap();
            file_identity(&file, &file.metadata().unwrap()).unwrap()
        };
        assert_eq!(id(&a), id(&a));
        assert_ne!(id(&a), id(&b));
    }

    #[test]
    fn remove_file_handles_read_only_files() {
        let (_temp, root) = root();
        let path = root.join("ro");
        fs::write(&path, b"x").unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions).unwrap();
        remove_file(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn directory_lock_is_exclusive_until_dropped() {
        let (_temp, root) = root();
        let held = try_lock_source(&root).unwrap().expect("uncontended");
        assert!(try_lock_source(&root).unwrap().is_none());
        drop(held);
        assert!(try_lock_source(&root).unwrap().is_some());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_rejected_at_the_leaf_and_in_ancestors() {
        use std::os::unix::fs::symlink;
        let (_temp, root) = root();
        let real = root.join("real");
        fs::create_dir(&real).unwrap();
        fs::write(real.join("file"), b"x").unwrap();
        let link = root.join("link");
        symlink(&real, &link).unwrap();
        assert!(!path_has_symlink_ancestor(&real.join("file")).unwrap());
        assert!(path_has_symlink_ancestor(&link.join("file")).unwrap());
        assert!(reject_symlinks(&link).is_err());
        assert!(reject_symlinks(&link.join("missing")).is_err());
        assert!(open_regular_read(&link.join("file")).is_err());
        assert!(open_log_append(&link.join("log")).is_err());
        // A dangling destination link is an existing entry, never replaced.
        let src = root.join("src");
        fs::write(&src, b"x").unwrap();
        let dangling = root.join("dangling");
        symlink(root.join("nowhere"), &dangling).unwrap();
        assert!(rename_noreplace(&src, &dangling).is_err());
        assert!(
            fs::symlink_metadata(&dangling)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(src.exists());
    }

    #[cfg(windows)]
    #[test]
    fn junctions_are_rejected_as_reparse_points() {
        let (_temp, root) = root();
        let real = root.join("real");
        fs::create_dir(&real).unwrap();
        fs::write(real.join("file"), b"x").unwrap();
        let junction = root.join("junction");
        // Junctions need no elevation, unlike Windows symlinks.
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&real)
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        assert!(is_link(&fs::symlink_metadata(&junction).unwrap()));
        assert!(path_has_symlink_ancestor(&junction.join("file")).unwrap());
        assert!(reject_symlinks(&junction).is_err());
        assert!(open_regular_read(&junction.join("file")).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn source_handles_block_writers_and_deletion() {
        let (_temp, root) = root();
        let path = root.join("busy");
        fs::write(&path, b"x").unwrap();
        let held = open_regular_read(&path).unwrap();
        assert!(OpenOptions::new().write(true).open(&path).is_err());
        assert!(fs::remove_file(&path).is_err());
        assert!(open_regular_read(&path).is_ok(), "readers may share");
        drop(held);
        fs::remove_file(&path).unwrap();
    }
}
