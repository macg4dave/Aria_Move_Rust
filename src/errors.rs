//! Typed error definitions for aria_move.
//! Small, focused set of well-known failure modes for better logs and tests.

use std::path::PathBuf;
use thiserror::Error;

/// Non-exhaustive to allow adding new variants without breaking downstream code.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AriaMoveError {
    /// The requested source path was not found.
    #[error("Source path not found: {0}")]
    SourceNotFound(PathBuf),

    /// A filesystem permission or policy prevents the operation.
    #[error("Permission denied on {path}: {context}")]
    PermissionDenied { path: PathBuf, context: String },

    /// Not enough free space at the destination for the operation.
    #[error("Insufficient disk space for destination {dest}: need {required} bytes, have {available} bytes")]
    InsufficientSpace {
        required: u128,
        available: u128,
        dest: PathBuf,
    },

    /// A cooperative shutdown/interrupt was requested (e.g., SIGINT).
    #[error("Operation interrupted by user")]
    Interrupted,

    // --- Resolution-specific errors ---
    /// Provided path exists but is not a regular file (e.g., directory, symlink if disallowed).
    #[error("Provided path is not a regular file: {0}")]
    ProvidedNotFile(PathBuf),
    /// Provided path existed initially but disappeared before use.
    #[error("Resolved file disappeared: {0}")]
    Disappeared(PathBuf),
    /// Automatic resolution found no candidate within the recency window.
    // Removed: RecentNotFound (fallback selects overall newest when none recent)
    /// Automatic resolution fell back (when allowed) and still found nothing at all.
    #[error("No file found under base: {0}")]
    NoneFound(PathBuf),
    /// Download base missing or not a directory.
    #[error("Download base invalid: {0}")]
    BaseInvalid(PathBuf),
}
