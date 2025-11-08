//! CLI definition and parsing.
//! Defines Args and provides parse() for command-line handling.
//!
//! Notes:
//! - --source-path takes precedence over the positional SOURCE_PATH (back-compat).
//! - --debug is a shorthand for --log-level debug.

use clap::{Parser, ValueHint};
use std::path::PathBuf;

use crate::config::types::{Config, LogLevel};

/// CLI wrapper for aria_move library.
/// CLI flags override config values (which are loaded from XML if present).
#[derive(Parser, Debug, Clone)]
#[command(
    author,
    version,
    about = "Move completed aria2 downloads safely (Rust)"
)]
pub struct Args {
    /// Aria2 task id (optional, informational). Ignored for auto-resolution logic.
    pub task_id: Option<String>,

    /// Number of files reported by aria2 (0 = unknown). Used only for heuristics around legacy positional path fallback.
    pub num_files: Option<usize>,

    /// Source path passed by aria2 (positional kept for compatibility).
    /// Prefer using `--source-path` for clarity; this positional is parsed only if present.
    #[arg(value_name = "SOURCE_PATH", value_hint = ValueHint::AnyPath)]
    pub source_path_pos: Option<PathBuf>,

    /// Explicit source path option — preferred way to specify the path; overrides positional.
    #[arg(
        long = "source-path",
        short = 's',
        value_name = "PATH",
        value_hint = ValueHint::AnyPath,
        help = "Source path (overrides positional)"
    )]
    pub source_path: Option<PathBuf>,

    /// Override the download base directory (normally configured via XML).
    #[arg(long, value_hint = ValueHint::DirPath, help = "Override the download base directory")]
    pub download_base: Option<PathBuf>,

    /// Override the completed base directory (normally configured via XML).
    #[arg(long, value_hint = ValueHint::DirPath, help = "Override the completed base directory")]
    pub completed_base: Option<PathBuf>,

    /// Enable debug logging (equivalent to `--log-level debug`).
    #[arg(
        short = 'd',
        long,
        help = "Enable debug logging (shorthand for --log-level debug)"
    )]
    pub debug: bool,

    /// Set log level. One of: quiet, normal, info, debug.
    #[arg(long, help = "Set log level: quiet, normal, info, debug")]
    pub log_level: Option<String>,

    /// Print where aria_move will look for the config file (or ARIA_MOVE_CONFIG if set), then exit.
    #[arg(
        long,
        help = "Print the config file location used by aria_move and exit"
    )]
    pub print_config: bool,

    /// Dry-run: log actions but do not modify the filesystem.
    #[arg(
        long,
        help = "Show what would be done, but do not modify files/directories"
    )]
    pub dry_run: bool,

    /// Preserve permissions, timestamps and xattrs (when feature enabled). Off by default.
    #[arg(
        long,
        help = "Preserve permissions, timestamps and xattrs (when enabled); slower"
    )]
    pub preserve_metadata: bool,

    /// Preserve only permissions (faster than full metadata). Ignored if --preserve-metadata is also set.
    #[arg(
        long,
        help = "Preserve only permissions (mode/readonly); faster than --preserve-metadata"
    )]
    pub preserve_permissions: bool,

    /// Emit logs in structured JSON (includes timestamp, level, and structured fields).
    #[arg(long, help = "Emit logs in structured JSON")]
    pub json: bool,
}

impl Args {
    /// Effective source path: `--source-path` if provided, else positional SOURCE_PATH.
    #[inline]
    /// Effective source path.
    ///
    /// Precedence:
    /// 1) `--source-path` if provided
    /// 2) positional `SOURCE_PATH` if provided
    /// 3) single positional first-argument (task_id) when the user invoked
    ///    `aria_move <filename>` (back-compat / convenience)
    pub fn resolved_source(&self) -> Option<std::path::PathBuf> {
        if let Some(p) = &self.source_path {
            return Some(p.clone());
        }
        if let Some(p) = &self.source_path_pos {
            return Some(p.clone());
        }

        // Stricter fallback: only treat `task_id` as a path when the user did
        // not provide `num_files` (i.e. they didn't invoke the aria2-style
        // three-argument form) and the `task_id` string looks like a path.
        // "Looks like a path" is a lightweight heuristic: contains a path
        // separator or a dot (e.g. "file.iso") or a drive-colon on Windows
        // (e.g. "C:\\file"). This avoids misinterpreting aria2 task IDs
        // (hash-like strings) as file paths.
        if self.num_files.is_none() && let Some(t) = &self.task_id && Self::looks_like_path(t) {
            return Some(std::path::PathBuf::from(t));
        }

        None
    }

    #[inline]
    fn looks_like_path(s: &str) -> bool {
        // Heuristic kept minimal: path separators, extension-like dot, drive-colon, or leading dot.
        s.contains('/') || s.contains('\\') || s.contains('.') || s.contains(':') || s.starts_with('.')
    }

    /// Effective log level derived from flags.
    /// Precedence: --debug > --log-level value > None (use config default).
    pub fn effective_log_level(&self) -> Option<LogLevel> {
        if self.debug {
            return Some(LogLevel::Debug);
        }
        self.log_level.as_deref().and_then(LogLevel::parse)
    }

    /// Apply CLI overrides to a loaded Config (in-place). No-ops for unset flags.
    pub fn apply_overrides(&self, cfg: &mut Config) {
        if let Some(db) = &self.download_base {
            cfg.download_base = db.clone();
        }
        if let Some(cb) = &self.completed_base {
            cfg.completed_base = cb.clone();
        }
        if let Some(level) = self.effective_log_level() {
            cfg.log_level = level;
        }
        if self.dry_run {
            cfg.dry_run = true;
        }
        if self.preserve_metadata {
            cfg.preserve_metadata = true;
        }
        if self.preserve_permissions {
            cfg.preserve_permissions = true;
        }
    }
}

pub fn parse() -> Args {
    Args::parse()
}
