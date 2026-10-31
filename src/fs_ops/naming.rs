//! Destination naming. An existing name is never reused: collisions get a
//! `-<millis>-<pid>[-n]` suffix before the extension, trimmed to fit the
//! filename limit. Publication itself is still exclusive, so this only picks
//! a name that is free now.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// Conservative per-component limits (UTF-16 units on Windows, bytes elsewhere).
#[cfg(windows)]
const MAX_NAME_UNITS: usize = 240;
#[cfg(unix)]
const MAX_NAME_UNITS: usize = 255;

/// `candidate` if nothing exists there, else a suffixed sibling that is free.
pub(super) fn unique_destination(candidate: &Path) -> PathBuf {
    if is_free(candidate) {
        return candidate.to_path_buf();
    }
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let tag = format!("-{millis}-{}", std::process::id());
    let stem = candidate.file_stem().unwrap_or(OsStr::new("file"));
    let ext = candidate.extension();
    (1u64..)
        .map(|n| {
            let suffix = if n == 1 {
                tag.clone()
            } else {
                format!("{tag}-{n}")
            };
            candidate.with_file_name(name_with_suffix(stem, ext, &suffix))
        })
        .find(|path| is_free(path))
        .expect("a finite directory cannot hold every suffix")
}

fn is_free(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_err()
}

fn units(s: &OsStr) -> usize {
    #[cfg(windows)]
    {
        std::os::windows::ffi::OsStrExt::encode_wide(s).count()
    }
    #[cfg(unix)]
    {
        s.len()
    }
}

/// Longest prefix of `value` within `budget` units, cut on a character
/// boundary when the name is valid Unicode.
fn truncate(value: &OsStr, budget: usize) -> OsString {
    if units(value) <= budget {
        return value.to_owned();
    }
    if let Some(text) = value.to_str() {
        let mut used = 0;
        let end = text
            .char_indices()
            .take_while(|(_, c)| {
                used += if cfg!(windows) {
                    c.len_utf16()
                } else {
                    c.len_utf8()
                };
                used <= budget
            })
            .last()
            .map_or(0, |(i, c)| i + c.len_utf8());
        return text[..end].into();
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        OsString::from_wide(&value.encode_wide().take(budget).collect::<Vec<_>>())
    }
    #[cfg(unix)]
    {
        // Any byte prefix of a Unix name is itself a valid name.
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        OsString::from_vec(value.as_bytes()[..budget].to_vec())
    }
}

/// `stem + suffix [+ "." + ext]`, trimming the stem (and if needed the
/// extension) so the whole name fits, while keeping the suffix intact.
fn name_with_suffix(stem: &OsStr, ext: Option<&OsStr>, suffix: &str) -> OsString {
    let suffix_units = units(OsStr::new(suffix));
    // Reserve one stem unit and the dot, even for an overlong extension.
    let ext = ext.map(|e| truncate(e, MAX_NAME_UNITS.saturating_sub(suffix_units + 2)));
    let ext_units = ext.as_deref().map_or(0, |e| 1 + units(e));
    let mut name = truncate(
        stem,
        MAX_NAME_UNITS.saturating_sub(suffix_units + ext_units),
    );
    if name.is_empty() {
        name.push("f");
    }
    name.push(suffix);
    if let Some(ext) = ext {
        name.push(".");
        name.push(ext);
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn free_names_are_kept() {
        let temp = tempfile::tempdir().unwrap();
        let candidate = temp.path().join("movie.mkv");
        assert_eq!(unique_destination(&candidate), candidate);
    }

    #[test]
    fn collisions_get_a_suffix_before_the_extension() {
        let temp = tempfile::tempdir().unwrap();
        let tag = format!("-{}", std::process::id());
        for (name, stem, ext) in [
            ("movie.mkv", "movie-", Some(".mkv")),
            ("archive.tar.gz", "archive.tar-", Some(".gz")),
            (".env", ".env-", None),
            ("folder", "folder-", None),
        ] {
            let candidate = temp.path().join(name);
            fs::write(&candidate, b"existing").unwrap();
            let unique = unique_destination(&candidate);
            let unique = unique.file_name().unwrap().to_str().unwrap();
            assert!(unique.starts_with(stem), "{unique}");
            assert!(unique.contains(&tag), "{unique}");
            match ext {
                Some(ext) => assert!(unique.ends_with(ext), "{unique}"),
                None => assert!(!unique[stem.len()..].contains('.'), "{unique}"),
            }
        }
    }

    #[test]
    fn repeated_collisions_count_up() {
        let temp = tempfile::tempdir().unwrap();
        let candidate = temp.path().join("a.txt");
        fs::write(&candidate, b"").unwrap();
        let first = unique_destination(&candidate);
        fs::write(&first, b"").unwrap();
        let second = unique_destination(&candidate);
        assert_ne!(first, second);
        assert!(is_free(&second));
    }

    #[test]
    fn long_names_are_trimmed_to_the_limit_keeping_suffix_and_extension() {
        let long = "é".repeat(400);
        for (stem, ext) in [
            (long.as_str(), Some("mkv")),
            ("x", Some(long.as_str())),
            (long.as_str(), None),
        ] {
            let name = name_with_suffix(OsStr::new(stem), ext.map(OsStr::new), "-123-45");
            assert!(units(&name) <= MAX_NAME_UNITS, "{} units", units(&name));
            let text = name.to_str().expect("cut on a char boundary");
            assert!(text.contains("-123-45"));
            if ext == Some("mkv") {
                assert!(text.ends_with("-123-45.mkv"));
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_names_are_trimmed_bytewise() {
        use std::os::unix::ffi::OsStrExt;
        let raw = vec![0xff; 300];
        let name = name_with_suffix(OsStr::from_bytes(&raw), Some(OsStr::new("bin")), "-1");
        assert_eq!(units(&name), MAX_NAME_UNITS);
        assert!(name.as_bytes().ends_with(b"-1.bin"));
    }
}
