use clap::Parser;
use std::path::PathBuf;

/// CLI wrapper for aria_move library.
/// CLI flags override config values (which are loaded from XML if present).
#[derive(Parser, Debug)]
#[command(author, version, about = "Move completed aria2 downloads safely (Rust)")]
pub struct Args {
    /// Aria2 task id (optional, informational)
    pub task_id: Option<String>,

    /// Number of files reported by aria2 (0 = unknown)
    pub num_files: Option<usize>,

    /// Source path passed by aria2
    pub source_path: Option<PathBuf>,

    /// Optional: override the download base (for testing)
    #[arg(long, help = "Override the download base directory")]
    pub download_base: Option<PathBuf>,

    /// Optional: override the completed base (for testing)
    #[arg(long, help = "Override the completed base directory")]
    pub completed_base: Option<PathBuf>,

    /// Enable debug logging (equivalent to `--log-level debug`)
    #[arg(short = 'd', long, help = "Enable debug logging (shorthand for --log-level debug)")]
    pub debug: bool,

    /// Set log level. One of: quiet, normal, info, debug
    #[arg(long, help = "Set log level: quiet, normal, info, debug")]
    pub log_level: Option<String>,

    /// Print where aria_move will look for the config file (or ARIA_MOVE_CONFIG if set), then exit.
    #[arg(long, help = "Print the config file location used by aria_move and exit")]
    pub print_config: bool,

    /// Dry-run: log actions but do not change filesystem.
    #[arg(long, help = "Show what would be done, but do not modify files/directories")]
    pub dry_run: bool,

    /// Preserve file permissions and mtime when moving (slower). Off by default.
    #[arg(long, help = "Preserve file permissions and mtime when moving (slower)")]
    pub preserve_metadata: bool,

    /// Emit logs in structured JSON (includes timestamp, level, fields)
    #[arg(long, help = "Emit logs in structured JSON")]
    pub json: bool,
}

pub fn parse() -> Args {
    Args::parse()
}