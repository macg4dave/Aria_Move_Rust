//! Free-space preflight for copies. Advisory only: real write failures still
//! propagate and leave the source untouched.
use crate::{AriaMoveError, platform};
use anyhow::Context;
use std::path::Path;

/// Headroom for metadata, journals and filesystem overhead.
const CUSHION_BYTES: u64 = 4 * 1024 * 1024;

/// Fail with `InsufficientSpace` unless `dir` has `required` bytes plus the cushion.
pub(super) fn ensure_space_for_copy(dir: &Path, required: u64) -> anyhow::Result<()> {
    let available = platform::free_space(dir)
        .with_context(|| format!("query free space at {}", dir.display()))?;
    check(dir, required, available)?;
    tracing::debug!(required, available, "space preflight passed");
    Ok(())
}

fn check(dir: &Path, required: u64, available: u64) -> Result<(), AriaMoveError> {
    let needed = required.saturating_add(CUSHION_BYTES);
    if available < needed {
        return Err(AriaMoveError::InsufficientSpace {
            required: needed,
            available,
            dest: dir.to_path_buf(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cushion_is_required_and_overflow_saturates() {
        let dir = Path::new("dest");
        assert!(check(dir, 0, CUSHION_BYTES).is_ok());
        assert!(check(dir, 1, CUSHION_BYTES + 1).is_ok());
        assert!(check(dir, u64::MAX - CUSHION_BYTES, u64::MAX).is_ok());
        assert_eq!(
            check(dir, 10, CUSHION_BYTES),
            Err(AriaMoveError::InsufficientSpace {
                required: CUSHION_BYTES + 10,
                available: CUSHION_BYTES,
                dest: dir.into(),
            })
        );
        assert!(check(dir, u64::MAX, u64::MAX - 1).is_err());
    }

    #[test]
    fn real_query_passes_for_a_tiny_copy_and_fails_for_an_impossible_one() {
        let temp = tempfile::tempdir().unwrap();
        ensure_space_for_copy(temp.path(), 1).unwrap();
        let error = ensure_space_for_copy(temp.path(), u64::MAX).unwrap_err();
        assert_eq!(crate::errors::error_code(&error), "insufficient_space");
    }
}
