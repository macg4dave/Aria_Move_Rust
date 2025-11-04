use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info, Level};
use tracing_subscriber::fmt::time::LocalTime;

use aria_move::{Config, move_entry};
use aria_move::ensure_default_config_exists;

/// CLI wrapper for aria_move library.
///
/// Keep the CLI minimal: config is read from disk by Config::default(),
/// CLI flags override those values. `--debug` is a convenient shorthand
/// for `--log-level debug`.
#[derive(Parser, Debug)]
#[command(author, version, about = "Move completed aria2 downloads safely (Rust)")]
struct Args {
    /// Aria2 task id (optional, informational)
    task_id: Option<String>,

    /// Number of files reported by aria2 (0 = unknown)
    num_files: Option<usize>,

    /// Source path passed by aria2
    source_path: Option<PathBuf>,

    /// Optional: override the download base (for testing)
    #[arg(long, help = "Override the download base directory")]
    download_base: Option<PathBuf>,

    /// Optional: override the completed base (for testing)
    #[arg(long, help = "Override the completed base directory")]
    completed_base: Option<PathBuf>,

    /// Enable debug logging (equivalent to `--log-level debug`)
    #[arg(short = 'd', long, help = "Enable debug logging (shorthand for --log-level debug)")]
    debug: bool,

    /// Set log level. One of: error, warn, info, debug, trace
    #[arg(long, help = "Set log level: error, warn, info, debug, trace")]
    log_level: Option<String>,
}

fn init_logging(level: Option<&str>) {
    let timer = LocalTime::rfc_3339();

    let lvl = match level.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("trace") => Level::TRACE,
        Some("debug") => Level::DEBUG,
        Some("warn") | Some("warning") => Level::WARN,
        Some("error") => Level::ERROR,
        _ => Level::INFO,
    };

    tracing_subscriber::fmt()
        .with_timer(timer)
        .with_target(false)
        .compact()
        .with_max_level(lvl)
        .init();
}

fn main() -> Result<()> {
    let args = Args::parse();

    // If there's no config file at the OS-default location, create a template
    // and inform the user so they can edit it. Exit so the user can populate
    // real values before the tool proceeds.
    if let Some(path) = ensure_default_config_exists() {
        println!("\nA template aria_move config was written to:\n  {}\n", path.display());
        println!("Edit the file to set download_base, completed_base and optionally log_level, for example:\n\n<config>\n  <download_base>/path/to/incoming</download_base>\n  <completed_base>/path/to/completed</completed_base>\n  <log_level>info</log_level>\n</config>\n");
        println!("Then re-run this command. To use a different location set ARIA_MOVE_CONFIG.\n");
        return Ok(());
    }

    // Build config (may read XML). CLI args override config values.
    let mut cfg = Config::default();
    // don't move values out of `args` (we still use `args` for logging later).
    if let Some(db) = args.download_base.as_ref() {
        cfg.download_base = db.clone();
    }
    if let Some(cb) = args.completed_base.as_ref() {
        cfg.completed_base = cb.clone();
    }

    // CLI log-level flags override the config/log file value.
    if let Some(lvl) = args.log_level.as_ref() {
        cfg.log_level = Some(lvl.clone());
    } else if args.debug {
        cfg.log_level = Some("debug".into());
    }

    // initialize logging with chosen level (config or default)
    init_logging(cfg.log_level.as_deref());

    info!("Starting aria_move: {:?}", args);

    // Validate paths and permissions before proceeding.
    if let Err(e) = cfg.validate() {
        error!("Configuration validation failed: {}", e);
        error!("Hint: check that download and completed directories exist and have correct permissions. Place config.xml in your config directory or set ARIA_MOVE_CONFIG to point to it.");
        return Err(e.into());
    }

    // Resolve source path (try provided path first, else find recent)
    let maybe_src = args.source_path.as_deref();
    let src = match aria_move::resolve_source_path(&cfg, maybe_src) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to resolve a source path: {:?}", e);
            return Err(e.into());
        }
    };

    match move_entry(&cfg, &src) {
        Ok(dest) => {
            info!(source = %src.display(), dest = %dest.display(), "Move completed");
            Ok(())
        }
        Err(e) => {
            error!(error = ?e, "Move failed");
            Err(e)
        }
    }
}
