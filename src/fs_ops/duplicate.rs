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
use tracing::trace;

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
            if !adjusted_candidate.exists() {
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
    if !candidate.exists() {
        return candidate;
    }

    // Try "stem (n).ext" for n = 2.. until free.
    let mut n: u64 = 2;
    let mut collisions = 0u32;
    const MAX_TRIES: u64 = 10_000;
    loop {
        let suffix = format!(" ({n})");
        let new_name = build_name_with_suffix(&stem, ext.as_deref(), &suffix);

        candidate = dst_dir.join(&new_name);
        if !candidate.exists() {
            return candidate;
        }
        collisions = collisions.saturating_add(1);
        if collisions == 3 {
            trace!(name = ?name, dir = %dst_dir.display(), "duplicate: experiencing multiple collisions, continuing to search unique suffix");
        }
        if n >= MAX_TRIES {
            // Final fallback if the directory is extremely crowded with numbered variants.
            let final_name = build_name_with_suffix(&stem, ext.as_deref(), " (final)");
            return dst_dir.join(final_name);
        }
        n = n.saturating_add(1);
    }
}

// Conservative filename limits (bytes/characters, platform-specific and approximate).
#[cfg(windows)]
const MAX_FILENAME_LEN: usize = 240; // leave headroom for legacy MAX_PATH
#[cfg(not(windows))]
const MAX_FILENAME_LEN: usize = 255; // typical POSIX/EXT limits

/// Measure the approximate length of an OsStr for filename budgeting.
#[cfg(unix)]
fn name_len_units(s: &OsStr) -> usize {
    use std::os::unix::ffi::OsStrExt;
    s.as_bytes().len()
}

#[cfg(windows)]
fn name_len_units(s: &OsStr) -> usize {
    // Best-effort: wide char count via lossy string.
    s.to_string_lossy().len()
}

/// Truncate the stem if needed to ensure `stem + suffix + ["." + ext]` fits within MAX_FILENAME_LEN.
fn build_name_with_suffix(stem: &OsStr, ext: Option<&OsStr>, suffix: &str) -> OsString {
    // Compute fixed overhead (suffix + optional "." + ext)
    let mut overhead = name_len_units(OsStr::new(suffix));
    let mut ext_part = OsString::new();
    if let Some(e) = ext {
        overhead = overhead.saturating_add(1 + name_len_units(e)); // dot + ext
        ext_part.push(".");
        ext_part.push(e);
    }

    let mut stem_os = stem.to_os_string();
    let name_len = name_len_units(&stem_os) + overhead;
    if name_len > MAX_FILENAME_LEN {
        // Need to shrink stem to fit
        let budget = MAX_FILENAME_LEN.saturating_sub(overhead);
        if budget == 0 {
            // Pathologically small budget; fall back to minimal marker
            stem_os = OsString::from("f");
        } else {
            // Try UTF-8-aware truncation first
            if let Some(stem_str) = stem.to_str() {
                let mut acc = String::new();
                for ch in stem_str.chars() {
                    acc.push(ch);
                    if name_len_units(OsStr::new(&acc)) > budget {
                        acc.pop();
                        break;
                    }
                }
                if acc.is_empty() {
                    // Ensure at least one character
                    acc.push('f');
                }
                stem_os = OsString::from(acc);
            } else {
                // Fallback: best-effort byte-wise truncation on Unix; on Windows use lossy string
                #[cfg(unix)]
                {
                    use std::os::unix::ffi::{OsStrExt, OsStringExt};
                    let bytes = stem.as_bytes();
                    let take = bytes.len().min(budget);
                    let taken = bytes[..take].to_vec();
                    stem_os = OsString::from_vec(taken);
                }
                #[cfg(windows)]
                {
                    let s = stem.to_string_lossy();
                    let mut acc = String::new();
                    for ch in s.chars() {
                        acc.push(ch);
                        if name_len_units(OsStr::new(&acc)) > budget {
                            acc.pop();
                            break;
                        }
                    }
                    if acc.is_empty() {
                        acc.push('f');
                    }
                    stem_os = OsString::from(acc);
                }
            }
        }
    }

    let mut new_name = OsString::new();
    new_name.push(&stem_os);
    if !suffix.is_empty() {
        new_name.push(suffix);
    }
    new_name.push(&ext_part);
    new_name
}
