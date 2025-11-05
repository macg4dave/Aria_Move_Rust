use std::path::PathBuf;
use std::time::Duration;

use super::paths::default_log_path;
use super::{DOWNLOAD_BASE_DEFAULT, COMPLETED_BASE_DEFAULT};

/// Program-defined verbosity levels exposed to users/config.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LogLevel {
    Quiet,   // only errors
    #[default]
    Normal,  // informational normal output
    Info,    // more info (like verbose)
    Debug,   // debug/trace
}

impl LogLevel {
    /// Parse common string names into our LogLevel.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "quiet" | "error" | "none" => Some(LogLevel::Quiet),
            "normal" | "info" => Some(LogLevel::Normal),
            "verbose" | "detailed" => Some(LogLevel::Info),
            "debug" | "trace" => Some(LogLevel::Debug),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub download_base: PathBuf,
    pub completed_base: PathBuf,
    pub log_level: LogLevel,
    pub log_file: Option<PathBuf>,
    pub dry_run: bool,
    pub preserve_metadata: bool,
    // how far back to consider "recent" files when auto-resolving source
    pub recent_window: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            download_base: PathBuf::from(DOWNLOAD_BASE_DEFAULT),
            completed_base: PathBuf::from(COMPLETED_BASE_DEFAULT),
            log_level: LogLevel::Normal,
            log_file: default_log_path(),
            dry_run: false,
            preserve_metadata: false,
            // default to 5 minutes of recency
            recent_window: Duration::from_secs(60 * 5),
        }
    }
}

impl Config {
    pub fn new(
        download_base: impl Into<PathBuf>,
        completed_base: impl Into<PathBuf>,
        recent_window: Duration,
    ) -> Self {
        Self {
            download_base: download_base.into(),
            completed_base: completed_base.into(),
            log_level: LogLevel::default(),
            log_file: default_log_path(),
            dry_run: false,
            preserve_metadata: false,
            recent_window,
        }
    }
}