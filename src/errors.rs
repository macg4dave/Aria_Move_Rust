//! Typed failure classes for logs, exit diagnostics and tests.
//!
//! Operations return `anyhow::Error` chains. Where a failure has a stable class,
//! an [`AriaMoveError`] is attached to the chain so callers can recover it with
//! `downcast_ref` while the original OS error stays available as the cause.

use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AriaMoveError {
    /// The requested source path does not exist.
    #[error("Source path not found: {0}")]
    SourceNotFound(PathBuf),

    /// The operating system denied access.
    #[error("Permission denied: cannot {operation} '{path}'")]
    PermissionDenied { operation: String, path: PathBuf },

    /// The destination filesystem lacks room for a copy.
    #[error("Insufficient disk space at {dest}: need {required} bytes, have {available} bytes")]
    InsufficientSpace {
        required: u64,
        available: u64,
        dest: PathBuf,
    },

    /// A cooperative shutdown (SIGINT/SIGTERM/Ctrl-C) was requested.
    #[error("Operation interrupted by user")]
    Interrupted,

    /// Any other I/O failure.
    #[error("Cannot {operation} '{path}'")]
    Io { operation: String, path: PathBuf },

    /// The source exists but is neither a regular file nor a directory.
    #[error("Provided path is not a regular file or directory: {0}")]
    ProvidedNotFile(PathBuf),

    /// No source path was supplied; nothing is ever selected automatically.
    #[error("No source path provided; pass the completed download (download base: {0})")]
    NoneFound(PathBuf),
}

impl AriaMoveError {
    /// Stable, machine-readable code for logs and JSON output.
    pub fn code(&self) -> &'static str {
        match self {
            Self::SourceNotFound(_) => "source_not_found",
            Self::PermissionDenied { .. } => "permission_denied",
            Self::InsufficientSpace { .. } => "insufficient_space",
            Self::Interrupted => "interrupted",
            Self::Io { .. } => "io_error",
            Self::ProvidedNotFile(_) => "provided_not_file",
            Self::NoneFound(_) => "none_found",
        }
    }
}

/// Attach the operation, path and stable class to an OS error, keeping it as the cause.
pub(crate) fn io_error(operation: &str, path: &Path, error: io::Error) -> anyhow::Error {
    let class = match error.kind() {
        io::ErrorKind::PermissionDenied => AriaMoveError::PermissionDenied {
            operation: operation.to_owned(),
            path: path.to_path_buf(),
        },
        io::ErrorKind::Interrupted => AriaMoveError::Interrupted,
        _ => AriaMoveError::Io {
            operation: operation.to_owned(),
            path: path.to_path_buf(),
        },
    };
    anyhow::Error::new(error).context(class)
}

/// Like [`io_error`], but a missing path is reported as a missing source.
pub(crate) fn source_error(path: &Path, error: io::Error) -> anyhow::Error {
    if error.kind() == io::ErrorKind::NotFound {
        anyhow::Error::new(error).context(AriaMoveError::SourceNotFound(path.to_path_buf()))
    } else {
        io_error("inspect source", path, error)
    }
}

/// The stable code of the outermost classified error in a chain.
pub fn error_code(error: &anyhow::Error) -> &'static str {
    // `downcast_ref` also searches context layers, which `chain()` cannot.
    error
        .downcast_ref::<AriaMoveError>()
        .map_or("error", AriaMoveError::code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_errors_are_classified_and_keep_their_cause() {
        let cases = [
            (io::ErrorKind::PermissionDenied, "permission_denied"),
            (io::ErrorKind::Interrupted, "interrupted"),
            (io::ErrorKind::NotFound, "io_error"),
            (io::ErrorKind::Other, "io_error"),
        ];
        for (kind, code) in cases {
            let error = io_error("open", Path::new("/x"), io::Error::from(kind));
            assert_eq!(error_code(&error), code, "{kind:?}");
            assert_eq!(
                error
                    .root_cause()
                    .downcast_ref::<io::Error>()
                    .unwrap()
                    .kind(),
                kind
            );
        }
    }

    #[test]
    fn missing_source_is_a_typed_not_found() {
        let error = source_error(Path::new("/gone"), io::ErrorKind::NotFound.into());
        assert_eq!(
            error.downcast_ref::<AriaMoveError>(),
            Some(&AriaMoveError::SourceNotFound(PathBuf::from("/gone")))
        );
        let error = source_error(Path::new("/x"), io::ErrorKind::PermissionDenied.into());
        assert_eq!(error_code(&error), "permission_denied");
    }

    #[test]
    fn messages_name_the_operation_and_path() {
        let error = io_error(
            "rename into",
            Path::new("/dest"),
            io::ErrorKind::Other.into(),
        );
        assert_eq!(error.to_string(), "Cannot rename into '/dest'");
        let rendered = format!("{error:#}");
        assert!(
            rendered.starts_with("Cannot rename into '/dest': "),
            "{rendered}"
        );
    }

    #[test]
    fn unclassified_chains_have_a_generic_code() {
        assert_eq!(error_code(&anyhow::anyhow!("plain")), "error");
        let nested = anyhow::Error::from(AriaMoveError::Interrupted).context("outer");
        assert_eq!(error_code(&nested), "interrupted");
    }
}
