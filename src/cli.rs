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

    /// Disable directory locking (for ZFS/NFS/network shares in containers where flock may fail).
    #[arg(
        long,
        help = "Disable directory locking (use for ZFS/NFS/network shares in containers)"
    )]
    pub disable_locks: bool,

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
    /// 3) single positional first-argument (task_id) is treated as the path when
    ///    the user invoked `aria_move <path>` (convenience). This is unconditional
    ///    when `num_files` and `SOURCE_PATH` are absent.
    pub fn resolved_source(&self) -> Option<std::path::PathBuf> {
        if let Some(p) = &self.source_path {
            return Some(Self::sanitize_path(p));
        }
        if let Some(p) = &self.source_path_pos {
            return Some(Self::sanitize_path(p));
        }

        // One-arg convenience: treat first positional as the path when the
        // aria2 three-argument form is not used and no SOURCE_PATH positional
        // was provided. We intentionally do NOT try to be clever here: any
        // single positional is interpreted as a path, and resolution will
        // fail later if it doesn't exist.
        if self.num_files.is_none()
            && self.source_path_pos.is_none()
            && let Some(t) = &self.task_id
        {
            return Some(Self::sanitize_str(t));
        }

        None
    }

    // Removed heuristic helper; we accept single positional as path unconditionally.
    #[inline]
    fn sanitize_path(p: &std::path::Path) -> PathBuf {
        Self::sanitize_str(&p.to_string_lossy())
    }

    #[inline]
    fn sanitize_str(s: &str) -> PathBuf {
        // Trim surrounding single/double quotes if user invoked with quotes in PowerShell or CMD.
        // Also trim any trailing unmatched quote caused by shell escaping mistakes.
        let trimmed = s.trim();
        let mut inner = if (trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        {
            trimmed[1..trimmed.len() - 1].to_string()
        } else {
            trimmed.trim_matches(|c| c == '\'' || c == '"').to_string()
        };

        // Remove any stray embedded quotes that may remain (e.g., "'path'/")
        inner.retain(|c| c != '\'' && c != '"');

        // Handle a trailing directory separator or backslash introduced by quoting/escaping.
        // Case 1: Windows/PowerShell often leaves a trailing backslash inside single quotes.
        // Case 2: Unix single quotes combined with a trailing slash may leave the trailing quote
        //         preserved incorrectly before sanitization (already trimmed) but leave an extra slash.
        // We remove ONE trailing slash or backslash if present to match existing test expectations.
        if inner.ends_with('\\') || inner.ends_with('/') {
            // Avoid stripping root "/" or "C:/" patterns inadvertently.
            if inner.len() > 1 {
                inner.pop();
            }
        }

        PathBuf::from(inner)
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
        if self.disable_locks {
            cfg.disable_locks = true;
        }
    }
}

pub fn parse() -> Args {
    Args::parse()
}
