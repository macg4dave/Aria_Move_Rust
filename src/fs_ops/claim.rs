//! Claim a source file by atomically renaming it in-place to a unique hidden name.
//! - Only one concurrent process can succeed (atomic rename in the same directory).
//! - Losers will see NotFound later and can exit gracefully if the destination exists.
//! - Name format: "<original>.aria_move.moving.<pid>.<nanos>[.<attempt>]"

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Atomically rename `src` to a unique hidden "claimed" name in the same directory.
/// Returns the claimed path on success.
/// Notes:
/// - Returns io::ErrorKind::NotFound if `src` no longer exists (race lost).
/// - May retry a few times if an unlikely name collision occurs.
pub(super) fn claim_source(src: &Path) -> io::Result<PathBuf> {
    let pid = std::process::id();
    // Base timestamp used in the suffix; attempt index is appended if we retry.
    let base_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let parent = src.parent().unwrap_or_else(|| Path::new("."));
    let fname = src.file_name().unwrap_or_else(|| OsStr::new("file"));

    // Try a few times in the astronomically unlikely event of a collision.
    const MAX_TRIES: u32 = 5;
    for attempt in 0..=MAX_TRIES {
        let mut new_name: OsString = fname.to_os_string();
        if attempt == 0 {
            new_name.push(format!(".aria_move.moving.{}.{}", pid, base_nanos));
        } else {
            new_name.push(format!(
                ".aria_move.moving.{}.{}.{}",
                pid, base_nanos, attempt
            ));
        }
        let claimed = parent.join(new_name);

        match fs::rename(src, &claimed) {
            Ok(()) => return Ok(claimed),
            Err(e) => {
                // If the source vanished, propagate NotFound (caller treats as race lost).
                if e.kind() == io::ErrorKind::NotFound {
                    return Err(e);
                }
                // If we somehow collided with an existing temp name, try another suffix.
                if e.kind() == io::ErrorKind::AlreadyExists && attempt < MAX_TRIES {
                    continue;
                }
                // Other errors (perm denied, sharing violation, etc.) bubble up.
                return Err(e);
            }
        }
    }

    // If we exhausted retries, fall back to a final rename attempt to surface the real error.
    let mut final_name: OsString = fname.to_os_string();
    final_name.push(format!(
        ".aria_move.moving.{}.{}.final",
        pid, base_nanos
    ));
    let final_claimed = parent.join(final_name);
    fs::rename(src, &final_claimed)?;
    Ok(final_claimed)
}