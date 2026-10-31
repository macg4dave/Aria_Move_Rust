//! Duplicate-name resolution utilities.
//!
//! Policy:
//! - Skip: return the intended path; caller should check for existence and skip if it exists.
//! - Overwrite: return the intended path; caller overwrites existing file if present.
//! - RenameWithSuffix: generate a unique name by appending " (n)" before the extension.
//!
//! Notes:
//! - This only decides the path name based on current filesystem state. Callers should still
//!   hold appropriate directory locks to avoid races with concurrent movers.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnDuplicate {
    /// Use the requested name; caller should skip operation if the path already exists.
    Skip,
    /// Return the requested name. This naming policy itself performs no writes;
    /// move/copy operations still refuse replacement.
    Overwrite,
    /// Pick a unique name by appending " (n)" before the extension.
    RenameWithSuffix,
}

/// Compute the destination filename according to the duplicate policy.
///
/// - dst_dir: target directory
/// - name: requested filename (including extension)
/// - policy: how to handle existing files
///
/// Returns a full path inside dst_dir. For Skip/Overwrite, this is simply dst_dir/name.
/// For RenameWithSuffix, a unique name is returned (dst_dir/name, name (2), name (3), ...).
pub fn resolve_destination(dst_dir: &Path, name: &OsStr, policy: OnDuplicate) -> PathBuf {
    let candidate = dst_dir.join(name);

    match policy {
        OnDuplicate::Skip | OnDuplicate::Overwrite => candidate,
        OnDuplicate::RenameWithSuffix => {
            // Do not suffix our own internal transient names; keep them as-is.
            if let Some(s) = name.to_str()
                && s.starts_with(".aria_move.")
            {
                return candidate;
            }
            // Path-length awareness: first, ensure the base name (without suffix) fits.
            let base = Path::new(name);
            let stem_os: OsString = base
                .file_stem()
                .map(|s| s.to_os_string())
                .unwrap_or_else(|| OsString::from(name));
            let ext_os: Option<OsString> = base.extension().map(|e| e.to_os_string());
            let adjusted_base = build_name_with_suffix(&stem_os, ext_os.as_deref(), "");
            let adjusted_candidate = dst_dir.join(&adjusted_base);
            if adjusted_candidate.symlink_metadata().is_err() {
                return adjusted_candidate;
            }
            unique_with_numeric_suffix(dst_dir, &adjusted_base)
        }
    }
}

/// Return a unique path by appending " (n)" before extension until no collision.
///
/// Examples:
/// - "movie.mkv" -> "movie (2).mkv", "movie (3).mkv", ...
/// - ".env" -> ".env (2)"
/// - "archive.tar.gz" -> "archive.tar (2).gz"
fn unique_with_numeric_suffix(dst_dir: &Path, name: &OsStr) -> PathBuf {
    let base = Path::new(name);

    // Extract stem and extension, preserving non-UTF8 via OsString.
    let stem: OsString = base
        .file_stem()
        .map(|s| s.to_os_string())
        .unwrap_or_else(|| OsString::from(name));
    let ext: Option<OsString> = base.extension().map(|e| e.to_os_string());

    // First try the requested name; if free, use it.
    let mut candidate = dst_dir.join(name);
    if candidate.symlink_metadata().is_err() {
        return candidate;
    }

    // Try "stem (n).ext" for n = 2.. until free.
    for n in 2u64.. {
        let suffix = format!(" ({n})");
        candidate = dst_dir.join(build_name_with_suffix(&stem, ext.as_deref(), &suffix));
        if std::fs::symlink_metadata(&candidate).is_err() {
            return candidate;
        }
    }
    // No unchecked final candidate; a finite directory cannot exhaust u64 names.
    candidate
}

/// Keep mover timestamp naming distinct from the public numeric policy while
/// sharing raw-name budgeting and collision detection.
pub(crate) fn unique_destination(candidate: &Path) -> PathBuf {
    if std::fs::symlink_metadata(candidate).is_err() {
        return candidate.to_path_buf();
    }
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let pid = std::process::id();
    let stem = candidate.file_stem().unwrap_or_else(|| OsStr::new("file"));
    let ext = candidate.extension();
    for n in 1u64.. {
        let suffix = if n == 1 {
            format!("-{millis}-{pid}")
        } else {
            format!("-{millis}-{pid}-{n}")
        };
        let path = candidate.with_file_name(build_name_with_suffix(stem, ext, &suffix));
        if std::fs::symlink_metadata(&path).is_err() {
            return path;
        }
    }
    candidate.to_path_buf() // Native no-replace publication still enforces safety.
}

// Conservative filename limits (bytes/characters, platform-specific and approximate).
#[cfg(windows)]
const MAX_FILENAME_LEN: usize = 240; // leave headroom for legacy MAX_PATH
#[cfg(not(windows))]
const MAX_FILENAME_LEN: usize = 255; // typical POSIX/EXT limits

/// Measure filesystem filename units without lossy conversion.
fn name_len_units(s: &OsStr) -> usize {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        s.as_bytes().len()
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        s.encode_wide().count()
    }
}

fn truncate_name(value: &OsStr, budget: usize) -> OsString {
    if name_len_units(value) <= budget {
        return value.to_owned();
    }
    if let Some(text) = value.to_str() {
        let mut result = OsString::new();
        for ch in text.chars() {
            let mut buffer = [0; 4];
            let part = OsStr::new(ch.encode_utf8(&mut buffer));
            if name_len_units(&result) + name_len_units(part) > budget {
                break;
            }
            result.push(part);
        }
        return result;
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        OsString::from_vec(value.as_bytes()[..budget].to_vec())
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        OsString::from_wide(&value.encode_wide().take(budget).collect::<Vec<_>>())
    }
}

fn build_name_with_suffix(stem: &OsStr, ext: Option<&OsStr>, suffix: &str) -> OsString {
    let suffix_len = name_len_units(OsStr::new(suffix));
    // Reserve at least one stem unit, including when an extension alone is overlong.
    let ext_budget = MAX_FILENAME_LEN.saturating_sub(suffix_len + 2);
    let ext = ext.map(|e| truncate_name(e, ext_budget));
    let overhead = suffix_len + ext.as_ref().map_or(0, |e| 1 + name_len_units(e));
    let mut name = truncate_name(stem, MAX_FILENAME_LEN.saturating_sub(overhead));
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
