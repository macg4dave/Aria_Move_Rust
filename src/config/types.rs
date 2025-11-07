//! Core configuration types.
//! - Config holds runtime settings with sensible defaults.
//! - LogLevel represents verbosity with simple parsing helpers.

use std::path::PathBuf;
use std::time::Duration;
use std::fmt;
use std::str::FromStr;

use super::{COMPLETED_BASE_DEFAULT, DOWNLOAD_BASE_DEFAULT};
use super::paths;

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
    /// Recency window for auto-resolving recent files
    pub recent_window: Duration,
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
            // Default to 5 minutes of recency
            recent_window: Duration::from_secs(60 * 5),
        }
    }
}

impl Config {
    /// Construct a Config with explicit bases and recency; other fields use defaults.
    pub fn new(
        download_base: impl Into<PathBuf>,
        completed_base: impl Into<PathBuf>,
        recent_window: Duration,
    ) -> Self {
        Self {
            download_base: download_base.into(),
            completed_base: completed_base.into(),
            recent_window,
            ..Default::default()
        }
    }
}
