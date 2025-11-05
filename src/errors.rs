//! Typed error definitions for aria_move.
//! Provides a small set of well-known failure modes for better logs and tests.

use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AriaMoveError {
    #[error("Source path not found: {0}")]
    SourceNotFound(PathBuf),

    #[error("Permission denied on {path}: {context}")]
    PermissionDenied { path: PathBuf, context: String },

    #[error("Insufficient disk space for destination {dest}: need {required} bytes, have {available} bytes")]
    InsufficientSpace {
        required: u128,
        available: u128,
        dest: PathBuf,
    },

    #[error("Operation interrupted by user")]
    Interrupted,
}
