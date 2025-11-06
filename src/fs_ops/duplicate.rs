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
    /// Use the requested name and overwrite if it already exists.
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
        OnDuplicate::RenameWithSuffix => unique_with_numeric_suffix(dst_dir, name),
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
    if !candidate.exists() {
        return candidate;
    }

    // Try "stem (n).ext" for n = 2.. until free.
    let mut n: u32 = 2;
    loop {
        let mut new_name = OsString::new();
        new_name.push(&stem);
        new_name.push(format!(" ({n})"));
        if let Some(ref e) = ext {
            new_name.push(".");
            new_name.push(e);
        }

        candidate = dst_dir.join(&new_name);
        if !candidate.exists() {
            return candidate;
        }
        n = n.saturating_add(1);
    }
}