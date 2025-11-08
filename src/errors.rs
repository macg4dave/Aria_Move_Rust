//! Typed error definitions for aria_move.
//! Small, focused set of well-known failure modes for better logs and tests.

use std::path::PathBuf;
use thiserror::Error;
use serde::{Serialize, Deserialize};
use std::io;

/// Non-exhaustive to allow adding new variants without breaking downstream code.
#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Automatic resolution found no candidate (respecting recent_window semantics).
    #[error("No file found under base: {0}")]
    NoneFound(PathBuf),
    /// Download base missing or not a directory.
    #[error("Download base invalid: {0}")]
    BaseInvalid(PathBuf),
}

impl AriaMoveError {
    /// Stable, machine-readable code for the variant (for logs/tests/JSON).
    pub fn code(&self) -> &'static str {
        match self {
            AriaMoveError::SourceNotFound(_) => "source_not_found",
            AriaMoveError::PermissionDenied { .. } => "permission_denied",
            AriaMoveError::InsufficientSpace { .. } => "insufficient_space",
            AriaMoveError::Interrupted => "interrupted",
            AriaMoveError::ProvidedNotFile(_) => "provided_not_file",
            AriaMoveError::Disappeared(_) => "disappeared",
            AriaMoveError::NoneFound(_) => "none_found",
            AriaMoveError::BaseInvalid(_) => "base_invalid",
        }
    }

    /// Map a std::io::Error that occurred while accessing `path` into a structured AriaMoveError.
    pub fn from_io(path: impl Into<PathBuf>, err: &io::Error) -> AriaMoveError {
        let path = path.into();
        match err.kind() {
            io::ErrorKind::NotFound => AriaMoveError::SourceNotFound(path),
            io::ErrorKind::PermissionDenied => AriaMoveError::PermissionDenied {
                path,
                context: "permission denied".to_string(),
            },
            _ => AriaMoveError::PermissionDenied {
                path,
                context: format!("io error: {:?}", err.kind()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};
    use std::path::PathBuf;

    #[test]
    fn code_mappings_are_stable() {
        assert_eq!(AriaMoveError::SourceNotFound(PathBuf::from("/x")).code(), "source_not_found");
        assert_eq!(AriaMoveError::PermissionDenied { path: PathBuf::from("/x"), context: "ro".into() }.code(), "permission_denied");
        assert_eq!(AriaMoveError::InsufficientSpace { required: 10, available: 5, dest: PathBuf::from("/dst") }.code(), "insufficient_space");
        assert_eq!(AriaMoveError::Interrupted.code(), "interrupted");
        assert_eq!(AriaMoveError::ProvidedNotFile(PathBuf::from("/x")).code(), "provided_not_file");
        assert_eq!(AriaMoveError::Disappeared(PathBuf::from("/x")).code(), "disappeared");
        assert_eq!(AriaMoveError::NoneFound(PathBuf::from("/base")).code(), "none_found");
        assert_eq!(AriaMoveError::BaseInvalid(PathBuf::from("/db")).code(), "base_invalid");
    }

    #[test]
    fn serializes_with_external_tag() {
        let e = AriaMoveError::PermissionDenied { path: PathBuf::from("/x"), context: "ro".into() };
        let v = to_value(&e).unwrap();
        // Externally tagged enum: { "PermissionDenied": { "path": "/x", "context": "ro" } }
        assert!(v.get("PermissionDenied").is_some());
        let inner = &v["PermissionDenied"];
        assert_eq!(inner["path"], json!(PathBuf::from("/x")));
        assert_eq!(inner["context"], json!("ro"));
    }

    #[test]
    fn maps_io_errors() {
        let nf = io::Error::from(io::ErrorKind::NotFound);
    let e = AriaMoveError::from_io("/missing", &nf);
    assert!(matches!(e, AriaMoveError::SourceNotFound(p) if p.as_path() == "/missing"));

        let pd = io::Error::from(io::ErrorKind::PermissionDenied);
    let e = AriaMoveError::from_io("/root", &pd);
    assert!(matches!(e, AriaMoveError::PermissionDenied { path, .. } if path.as_path() == "/root"));

        let other = io::Error::from(io::ErrorKind::Other);
        let e = AriaMoveError::from_io("/x", &other);
        if let AriaMoveError::PermissionDenied { context, .. } = e {
            assert!(context.contains("io error"));
        } else {
            panic!("expected PermissionDenied for Other kind");
        }
    }
}
