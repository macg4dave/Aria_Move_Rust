//! Exclusive native rename, shared by file moves, tree moves and publication.
use anyhow::Result;
use std::path::Path;

/// Outcome of attempting an exclusive atomic move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveOutcome {
    Renamed,
    CrossDevice,
}

/// Rename without replacing any existing destination. Only cross-device failure
/// requests a copy fallback; all other errors retain their original cause.
pub fn try_atomic_move(src: &Path, dst: &Path) -> Result<MoveOutcome> {
    crate::shutdown::check()?;
    match crate::platform::rename_noreplace(src, dst) {
        Ok(()) => {
            for parent in [dst.parent(), src.parent()].into_iter().flatten() {
                // Rename already committed. A sync failure cannot safely roll it back.
                if let Err(e) = crate::platform::sync_directory(parent) {
                    tracing::warn!(error = %e, path = %parent.display(), "rename committed; directory sync failed");
                }
            }
            Ok(MoveOutcome::Renamed)
        }
        Err(e) if super::util::is_cross_device(&e) => Ok(MoveOutcome::CrossDevice),
        Err(e) => {
            let operation = format!("atomic rename from '{}'", src.display());
            Err(super::helpers::io_error_with_help(&operation, dst)(e))
        }
    }
}
