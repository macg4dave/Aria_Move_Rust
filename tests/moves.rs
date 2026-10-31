//! Library moves under real filesystem conditions: a second filesystem,
//! permissions, contention and platform-specific sharing rules.
mod common;

use aria_move::{Config, move_entry};
use common::Fixture;
use std::fs;

fn config(fx: &Fixture) -> Config {
    Config::new(&fx.download, &fx.completed)
}

/// Real cross-filesystem moves, exercising the verified copy path end to end.
/// Skipped where the machine offers no second filesystem.
#[test]
fn cross_filesystem_moves_are_complete_and_remove_the_source() {
    let Some(other) = common::other_filesystem() else {
        eprintln!("skipped: no second filesystem available");
        return;
    };
    let fx = Fixture::new();
    let completed = dunce::canonicalize(other.path()).unwrap().join("completed");
    let mut cfg = Config::new(&fx.download, &completed);
    cfg.preserve_metadata = true;

    let bytes: Vec<u8> = (0..5_000_000u32).map(|n| (n % 241) as u8).collect();
    let file = fx.file("big.bin", &bytes);
    let modified = filetime::FileTime::from_unix_time(1_700_000_000, 0);
    filetime::set_file_mtime(&file, modified).unwrap();
    fx.file("tree/nested/child", b"child bytes");
    fs::create_dir_all(fx.download.join("tree/empty")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
        fs::set_permissions(fx.download.join("tree"), fs::Permissions::from_mode(0o750)).unwrap();
    }
    #[cfg(all(unix, feature = "xattrs"))]
    let has_xattrs = xattr::set(
        fx.download.join("tree/nested/child"),
        "user.aria_move",
        b"v",
    )
    .is_ok();

    let dest_file = move_entry(&cfg, &file).unwrap();
    let dest_tree = move_entry(&cfg, &fx.download.join("tree")).unwrap();

    assert_eq!(fs::read(&dest_file).unwrap(), bytes);
    let meta = fs::metadata(&dest_file).unwrap();
    assert_eq!(
        filetime::FileTime::from_last_modification_time(&meta),
        modified
    );
    assert_eq!(
        fs::read(dest_tree.join("nested/child")).unwrap(),
        b"child bytes"
    );
    assert!(dest_tree.join("empty").is_dir());
    assert!(common::names(&fx.download).is_empty(), "sources removed");
    assert_eq!(
        common::names(&completed),
        ["big.bin", "tree"],
        "no staging left behind"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(meta.permissions().mode() & 0o777, 0o640);
        assert_eq!(
            fs::metadata(&dest_tree).unwrap().permissions().mode() & 0o777,
            0o750
        );
    }
    #[cfg(all(unix, feature = "xattrs"))]
    if has_xattrs {
        let value = xattr::get(dest_tree.join("nested/child"), "user.aria_move").unwrap();
        assert_eq!(value.as_deref(), Some(&b"v"[..]));
    }
}

/// Concurrent movers of same-named entries must each get a distinct, complete
/// destination; none may replace another.
#[test]
fn concurrent_moves_of_the_same_name_never_collide() {
    let fx = Fixture::new();
    let cfg = config(&fx);
    let sources: Vec<_> = (0..8)
        .map(|i| {
            fx.file(
                &format!("batch{i}/same.bin"),
                format!("payload {i}").as_bytes(),
            )
        })
        .collect();
    let barrier = std::sync::Barrier::new(sources.len());
    let destinations: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = sources
            .iter()
            .map(|src| {
                scope.spawn(|| {
                    barrier.wait();
                    move_entry(&cfg, src).unwrap()
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut contents: Vec<_> = destinations
        .iter()
        .map(|d| fs::read_to_string(d).unwrap())
        .collect();
    contents.sort();
    let expected: Vec<_> = (0..8).map(|i| format!("payload {i}")).collect();
    assert_eq!(contents, expected);
    assert_eq!(fx.completed_names().len(), 8);
}

#[test]
fn moves_into_a_missing_completed_base_create_it() {
    let fx = Fixture::new();
    let completed = fx.root.join("new").join("completed");
    let cfg = Config::new(&fx.download, &completed);
    let file = fx.file("f", b"x");
    assert_eq!(move_entry(&cfg, &file).unwrap(), completed.join("f"));
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn set_mode(path: &std::path::Path, mode: u32) {
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn an_unwritable_destination_is_a_typed_permission_error() {
        if common::is_root() {
            return;
        }
        let fx = Fixture::new();
        let file = fx.file("sample.txt", b"hello");
        set_mode(&fx.completed, 0o555);
        let error = move_entry(&config(&fx), &file).unwrap_err();
        set_mode(&fx.completed, 0o700);
        assert_eq!(
            aria_move::errors::error_code(&error),
            "permission_denied",
            "{error:#}"
        );
        assert_eq!(fs::read(&file).unwrap(), b"hello");
        assert!(fx.completed_names().is_empty());
    }

    #[test]
    fn locks_need_readable_directories_unless_disabled() {
        if common::is_root() {
            return;
        }
        for (directory_is_source, entry) in
            [(true, "f.bin"), (false, "f.bin"), (false, "folder/a.txt")]
        {
            let fx = Fixture::new();
            let path = fx.file(entry, b"data");
            let src = fx.download.join(entry.split('/').next().unwrap());
            let unreadable = if directory_is_source {
                &fx.download
            } else {
                &fx.completed
            };
            set_mode(unreadable, 0o333); // write+search, no read: cannot be opened to lock
            let locked = move_entry(&config(&fx), &src);
            let mut cfg = config(&fx);
            cfg.disable_locks = true;
            let unlocked = move_entry(&cfg, &src);
            set_mode(unreadable, 0o700);
            assert!(locked.is_err(), "{entry}");
            let dest = unlocked.unwrap_or_else(|e| panic!("{entry}: {e:#}"));
            assert!(dest.exists() && !path.exists());
        }
    }
}

#[cfg(windows)]
mod windows {
    use super::*;

    /// aria2 on Windows keeps a download open for writing until it finishes;
    /// such a file must be refused, never half-moved.
    #[test]
    fn a_file_open_for_writing_is_refused_untouched() {
        let fx = Fixture::new();
        let file = fx.file("downloading.bin", b"partial");
        let writer = fs::OpenOptions::new().write(true).open(&file).unwrap();
        let result = move_entry(&config(&fx), &file);
        drop(writer);
        assert!(result.is_err());
        assert_eq!(fs::read(&file).unwrap(), b"partial");
        assert!(fx.completed_names().is_empty());
    }
}
