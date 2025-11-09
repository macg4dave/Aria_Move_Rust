//! Core configuration types.
//! - Config holds runtime settings with sensible defaults.
//! - LogLevel represents verbosity with simple parsing helpers.

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use super::paths;
use super::{COMPLETED_BASE_DEFAULT, DOWNLOAD_BASE_DEFAULT};

/// Program-defined verbosity levels exposed to users/config.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LogLevel {
    /// Only errors
    Quiet,
    /// Informational output (default)
    #[default]
    Normal,
    /// More info (like verbose)
    Info,
    /// Debug/trace
    Debug,
}

impl LogLevel {
    /// Parse common string names into our LogLevel (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "quiet" | "error" | "none" => Some(LogLevel::Quiet),
            "normal" => Some(LogLevel::Normal),
            "info" | "verbose" | "detailed" => Some(LogLevel::Info),
            "debug" | "trace" => Some(LogLevel::Debug),
            _ => None,
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            LogLevel::Quiet => "quiet",
            LogLevel::Normal => "normal",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
        };
        f.write_str(s)
    }
}

impl FromStr for LogLevel {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("invalid log level: '{s}'"))
    }
}

/// Runtime configuration used by the mover.
#[derive(Debug, Clone)]
pub struct Config {
    /// Where partial/new downloads appear
    pub download_base: PathBuf,
    /// Final destination for completed items
    pub completed_base: PathBuf,
    /// Console verbosity
    pub log_level: LogLevel,
    /// Optional path to a log file
    pub log_file: Option<PathBuf>,
    /// If true, print actions but do not modify the filesystem
    pub dry_run: bool,
    /// If true, preserve permissions and timestamps
    pub preserve_metadata: bool,
    /// If true, preserve only permissions (mode / readonly). Ignored if preserve_metadata is true.
    pub preserve_permissions: bool,
    /// If true, disable directory locking (for ZFS/NFS/network shares in containers)
    pub disable_locks: bool,
    // Single switch: when true, preserve all available metadata (times, perms, readonly, xattrs).
    // When false, preserve nothing.
    // (auto-pick recency window removed; explicit source path required)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            download_base: PathBuf::from(DOWNLOAD_BASE_DEFAULT),
            completed_base: PathBuf::from(COMPLETED_BASE_DEFAULT),
            log_level: LogLevel::Normal,
            // paths::default_log_path() returns Result<PathBuf>; store Some(path) on success.
            log_file: paths::default_log_path().ok(),
            dry_run: false,
            preserve_metadata: false,
            preserve_permissions: false,
            disable_locks: false,
            // no auto-pick window
        }
    }
}

impl Config {
    /// Construct a Config with explicit bases; other fields use defaults.
    pub fn new(download_base: impl Into<PathBuf>, completed_base: impl Into<PathBuf>) -> Self {
        Self {
            download_base: download_base.into(),
            completed_base: completed_base.into(),
            ..Default::default()
        }
    }
}
