//! Shared copy-space policy and byte formatting. Platform owns the syscall.
//! Queries are advisory: actual write failures still propagate and preserve sources.
use crate::errors::AriaMoveError;
use std::path::Path;

/// Binary-unit formatting (KiB/MiB/GiB) rounded to one decimal; trims trailing `.0`.
pub(super) fn format_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let f = n as f64;
    let (value, unit) = if f >= GB {
        (f / GB, "GiB")
    } else if f >= MB {
        (f / MB, "MiB")
    } else if f >= KB {
        (f / KB, "KiB")
    } else {
        return format!("{} B", n);
    };
    let formatted = format!("{:.1}", value);
    let trimmed = if formatted.ends_with(".0") {
        &formatted[..formatted.len() - 2]
    } else {
        &formatted
    };
    format!("{} {}", trimmed, unit)
}

/// Fixed cushion in bytes added to required amount when validating free space.
pub(super) const SPACE_CUSHION_BYTES: u64 = 4 * 1024 * 1024; // 4 MiB

/// Pure function: returns true if `free` bytes is >= required + cushion.
pub(super) fn has_space(free: u64, required: u64) -> bool {
    required
        .checked_add(SPACE_CUSHION_BYTES)
        .is_some_and(|need| free >= need)
}

/// Ensure the destination filesystem has at least `required` bytes plus a small cushion.
/// The cushion helps avoid borderline failures from metadata, journal, and temp usage.
pub(super) fn ensure_space_for_copy(dst_dir: &Path, required: u64) -> anyhow::Result<()> {
    use anyhow::Context;
    let mut query = dst_dir;
    while !query.is_dir() {
        query = query
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        if query == Path::new(".") {
            break;
        }
    }
    let free = crate::platform::check_disk_space(query)
        .with_context(|| format!("query free space at {}", query.display()))?;
    if !has_space(free, required) {
        return Err(AriaMoveError::InsufficientSpace {
            required: required as u128 + SPACE_CUSHION_BYTES as u128,
            available: free as u128,
            dest: dst_dir.to_path_buf(),
        }
        .into());
    }
    tracing::debug!(required = %format_bytes(required), available = %format_bytes(free), "space preflight");
    Ok(())
}

// ---------- Tests ----------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_boundaries() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1 KiB");
        assert_eq!(format_bytes(1024 * 1024), "1 MiB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5 MiB");
    }

    #[test]
    fn has_space_logic() {
        let cushion = SPACE_CUSHION_BYTES;
        assert!(has_space(cushion, 0)); // Free == cushion, required 0
        assert!(has_space(cushion + 1, 1));
        assert!(!has_space(cushion - 1, 0));
        // Near saturation
        let max = u64::MAX;
        assert!(has_space(max, max - cushion));
    }

    #[test]
    fn ensure_space_for_copy_parent_fallback() {
        // Use a temp directory and pass a prospective file path (non-existent).
        let dir = tempfile::tempdir().unwrap();
        let prospective = dir.path().join("future_file.bin");
        // Required 1 byte should pass (unless disk is almost full).
        ensure_space_for_copy(&prospective, 1).unwrap();
    }

    // Helper to exercise the error path deterministically without relying on actual disk space.
    #[track_caller]
    fn simulate_insufficient(
        query_path: &Path,
        free: u64,
        required: u64,
    ) -> Result<(), AriaMoveError> {
        if !has_space(free, required) {
            return Err(AriaMoveError::InsufficientSpace {
                required: (required as u128).saturating_add(SPACE_CUSHION_BYTES as u128),
                available: free as u128,
                dest: query_path.to_path_buf(),
            });
        }
        Ok(())
    }

    #[test]
    fn insufficient_space_error_variant() {
        let dir = tempfile::tempdir().unwrap();
        let prospective = dir.path().join("file.bin");
        // Choose free smaller than cushion to force error for any required > 0
        let free = SPACE_CUSHION_BYTES - 1;
        let required = 1u64;
        let err = simulate_insufficient(&prospective, free, required).unwrap_err();
        match err {
            AriaMoveError::InsufficientSpace {
                required: need,
                available,
                dest,
            } => {
                assert_eq!(need, (required as u128) + (SPACE_CUSHION_BYTES as u128));
                assert_eq!(available, free as u128);
                assert_eq!(dest, prospective);
            }
            _ => panic!("unexpected error variant"),
        }
    }
}
