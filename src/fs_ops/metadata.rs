//! Best-effort metadata preservation for copies. A rename keeps everything,
//! so these only run on the copy fallback. Failures are logged, never fatal.
use crate::Config;
use std::fs::{self, Metadata};
use std::path::Path;
use tracing::warn;

/// Apply the configured metadata policy from `meta` (the source's) to `dest`.
/// Permissions go last: a read-only result would block the other writes.
pub(super) fn apply(config: &Config, src: &Path, dest: &Path, meta: &Metadata) {
    if config.preserve_metadata {
        copy_times(dest, meta);
        copy_xattrs(src, dest);
        copy_permissions(dest, meta);
    } else if config.preserve_permissions {
        copy_permissions(dest, meta);
    }
}

/// Unix mode bits (without setuid/setgid/sticky) or the Windows read-only flag.
fn copy_permissions(dest: &Path, meta: &Metadata) {
    #[cfg(unix)]
    let permissions = {
        use std::os::unix::fs::PermissionsExt;
        fs::Permissions::from_mode(meta.permissions().mode() & 0o777)
    };
    #[cfg(windows)]
    let permissions = meta.permissions();
    if let Err(error) = fs::set_permissions(dest, permissions) {
        warn!(path = %dest.display(), %error, "could not preserve permissions");
    }
}

fn copy_times(dest: &Path, meta: &Metadata) {
    let accessed = filetime::FileTime::from_last_access_time(meta);
    let modified = filetime::FileTime::from_last_modification_time(meta);
    if let Err(error) = filetime::set_file_times(dest, accessed, modified) {
        warn!(path = %dest.display(), %error, "could not preserve timestamps");
    }
}

#[cfg(all(unix, feature = "xattrs"))]
fn copy_xattrs(src: &Path, dest: &Path) {
    let names = match xattr::list(src) {
        Ok(names) => names,
        Err(error) => {
            warn!(path = %src.display(), %error, "could not list extended attributes");
            return;
        }
    };
    for name in names {
        let result = xattr::get(src, &name)
            .and_then(|value| xattr::set(dest, &name, value.as_deref().unwrap_or_default()));
        if let Err(error) = result {
            warn!(path = %dest.display(), xattr = %name.to_string_lossy(), %error, "could not preserve extended attribute");
        }
    }
}

#[cfg(not(all(unix, feature = "xattrs")))]
fn copy_xattrs(_src: &Path, _dest: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_controls_what_is_copied() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("src");
        fs::write(&src, b"x").unwrap();
        let old = filetime::FileTime::from_unix_time(1_600_000_000, 0);
        filetime::set_file_mtime(&src, old).unwrap();
        let mut readonly = fs::metadata(&src).unwrap().permissions();
        readonly.set_readonly(true);
        fs::set_permissions(&src, readonly).unwrap();
        let meta = fs::metadata(&src).unwrap();

        // (preserve_metadata, preserve_permissions) -> (read-only copied, mtime copied)
        let cases = [
            ((false, false), (false, false)),
            ((false, true), (true, false)),
            ((true, false), (true, true)),
            ((true, true), (true, true)),
        ];
        for (index, ((preserve_metadata, preserve_permissions), expected)) in
            cases.into_iter().enumerate()
        {
            // A fresh, writable destination, exactly like a staged copy.
            let dest = temp.path().join(format!("dest{index}"));
            fs::write(&dest, b"x").unwrap();
            let cfg = Config {
                preserve_metadata,
                preserve_permissions,
                ..Config::default()
            };
            apply(&cfg, &src, &dest, &meta);
            let result = fs::metadata(&dest).unwrap();
            let actual = (
                result.permissions().readonly(),
                filetime::FileTime::from_last_modification_time(&result) == old,
            );
            assert_eq!(
                actual, expected,
                "metadata={preserve_metadata} permissions={preserve_permissions}"
            );
            crate::platform::remove_file(&dest).unwrap();
        }
        crate::platform::remove_file(&src).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn unix_mode_bits_are_copied_without_special_bits() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let (src, dest) = (temp.path().join("src"), temp.path().join("dest"));
        fs::write(&src, b"x").unwrap();
        fs::write(&dest, b"x").unwrap();
        fs::set_permissions(&src, fs::Permissions::from_mode(0o4640)).unwrap();
        copy_permissions(&dest, &fs::metadata(&src).unwrap());
        assert_eq!(
            fs::metadata(&dest).unwrap().permissions().mode() & 0o7777,
            0o640
        );
    }

    #[cfg(all(unix, feature = "xattrs"))]
    #[test]
    fn extended_attributes_are_copied() {
        let temp = tempfile::tempdir().unwrap();
        let (src, dest) = (temp.path().join("src"), temp.path().join("dest"));
        fs::write(&src, b"x").unwrap();
        fs::write(&dest, b"x").unwrap();
        if xattr::set(&src, "user.aria_move", b"value").is_err() {
            return; // filesystem without user xattrs (e.g. some tmpfs builds)
        }
        copy_xattrs(&src, &dest);
        assert_eq!(
            xattr::get(&dest, "user.aria_move").unwrap().as_deref(),
            Some(&b"value"[..])
        );
    }
}
