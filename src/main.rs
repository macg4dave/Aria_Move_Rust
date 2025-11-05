use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;
use tracing_appender::non_blocking::WorkerGuard;
use chrono::Local;
use std::fmt as stdfmt;
use std::sync::{Arc, Mutex};
use aria_move::shutdown;

use aria_move::{Config, LogLevel, move_entry, validate_paths, ensure_default_config_exists, default_config_path, path_has_symlink_ancestor};
use std::env;
use std::os::unix::fs::OpenOptionsExt as MainOpenOptionsExt; // keep local alias for clarity

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

    /// Set log level. One of: quiet, normal, info, debug
    #[arg(long, help = "Set log level: quiet, normal, info, debug")]
    log_level: Option<String>,

    /// Print where aria_move will look for the config file (or ARIA_MOVE_CONFIG if set), then exit.
    #[arg(long, help = "Print the config file location used by aria_move and exit")]
    print_config: bool,

    /// Dry-run: log actions but do not change filesystem.
    #[arg(long, help = "Show what would be done, but do not modify files/directories")]
    dry_run: bool,

    /// Preserve file permissions and mtime when moving (slower). Off by default.
    #[arg(long, help = "Preserve file permissions and mtime when moving (slower)")]
    preserve_metadata: bool,

    /// Emit logs in structured JSON (includes timestamp, level, fields)
    #[arg(long, help = "Emit logs in structured JSON")]
    json: bool,
}

// small custom timer to print timestamps in user's human-readable format: DD/MM/YY HH:MM:SS
struct LocalHumanTime;

impl FormatTime for LocalHumanTime {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> stdfmt::Result {
        let now = Local::now();
        // %d/%m/%y %H:%M:%S -> 04/11/25 16:04:13
        write!(w, "{}", now.format("%d/%m/%y %H:%M:%S"))
    }
}

/// Initialize tracing based on our LogLevel. Returns an optional WorkerGuard if a file
/// appender was created; caller should drop the guard before exit to ensure logs are written.
fn init_logging_level(
    lvl: &LogLevel,
    log_file: Option<&std::path::Path>,
    json: bool,
) -> Result<Option<WorkerGuard>> {
    use tracing_subscriber::fmt as tsfmt;

    // build tracing level filter from our LogLevel
    let level_filter = match lvl {
        LogLevel::Quiet => LevelFilter::ERROR,
        LogLevel::Normal => LevelFilter::INFO,
        LogLevel::Info => LevelFilter::DEBUG,
        LogLevel::Debug => LevelFilter::TRACE,
    };

    // Branch on JSON vs pretty to avoid mixed generic types.
    if json {
        // stdout JSON layer
        let stdout_layer = tsfmt::layer()
            .json()
            .with_timer(LocalHumanTime)
            .with_level(true)
            .with_target(false)
            .with_filter(level_filter);

        // If file logging requested, add a JSON file layer as well.
        if let Some(path) = log_file {
            // security: refuse file logging if any existing ancestor is a symlink
            match path_has_symlink_ancestor(path) {
                Ok(true) => {
                    eprintln!("Refusing to enable file logging: existing ancestor of {} is a symlink; proceeding without file logging.", path.display());
                    registry().with(stdout_layer).init();
                    return Ok(None);
                }
                Err(e) => {
                    eprintln!("Error checking log path {} for symlinks: {}; proceeding without file logging.", path.display(), e);
                    registry().with(stdout_layer).init();
                    return Ok(None);
                }
                Ok(false) => {}
            }

            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }

            // open log file safely
            #[cfg(unix)]
            let file = {
                let mut opts = std::fs::OpenOptions::new();
                opts.create(true).append(true).mode(0o600).custom_flags(libc::O_NOFOLLOW);
                opts.open(path).map_err(|e| anyhow::anyhow!("Failed to open log file {}: {}", path.display(), e))?
            };
            #[cfg(not(unix))]
            let file = {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .map_err(|e| anyhow::anyhow!("Failed to open log file {}: {}", path.display(), e))?
            };

            let (writer, guard) = tracing_appender::non_blocking(file);

            let file_layer = tsfmt::layer()
                .json()
                .with_timer(LocalHumanTime)
                .with_level(true)
                .with_target(false)
                .with_writer(writer)
                .with_filter(level_filter);

            registry().with(stdout_layer).with(file_layer).init();
            return Ok(Some(guard));
        } else {
            // stdout only (JSON)
            registry().with(stdout_layer).init();
            return Ok(None);
        }
    } else {
        // Pretty/compact text for both stdout and file (consistent fields)
        let stdout_layer = tsfmt::layer()
            .with_timer(LocalHumanTime)
            .with_level(true)
            .with_target(false)
            .compact()
            .with_filter(level_filter);

        if let Some(path) = log_file {
            // security: refuse file logging if any existing ancestor is a symlink
            match path_has_symlink_ancestor(path) {
                Ok(true) => {
                    eprintln!("Refusing to enable file logging: existing ancestor of {} is a symlink; proceeding without file logging.", path.display());
                    registry().with(stdout_layer).init();
                    return Ok(None);
                }
                Err(e) => {
                    eprintln!("Error checking log path {} for symlinks: {}; proceeding without file logging.", path.display(), e);
                    registry().with(stdout_layer).init();
                    return Ok(None);
                }
                Ok(false) => {}
            }

            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }

            #[cfg(unix)]
            let file = {
                let mut opts = std::fs::OpenOptions::new();
                opts.create(true).append(true).mode(0o600).custom_flags(libc::O_NOFOLLOW);
                opts.open(path).map_err(|e| anyhow::anyhow!("Failed to open log file {}: {}", path.display(), e))?
            };
            #[cfg(not(unix))]
            let file = {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .map_err(|e| anyhow::anyhow!("Failed to open log file {}: {}", path.display(), e))?
            };

            let (writer, guard) = tracing_appender::non_blocking(file);

            let file_layer = tsfmt::layer()
                .with_timer(LocalHumanTime)
                .with_level(true)
                .with_target(false)
                .compact()
                .with_writer(writer)
                .with_filter(level_filter);

            registry().with(stdout_layer).with(file_layer).init();
            return Ok(Some(guard));
        } else {
            registry().with(stdout_layer).init();
            return Ok(None);
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Print config location request is handled before logging init
    if args.print_config {
        if let Ok(cfg_env) = env::var("ARIA_MOVE_CONFIG") {
            println!("Using ARIA_MOVE_CONFIG (explicit):\n  {}\n", cfg_env);
            println!("To override, unset ARIA_MOVE_CONFIG or set it to another file.");
            return Ok(());
        }

        match default_config_path() {
            Some(p) => {
                println!("Default aria_move config path:\n  {}\n", p.display());
                if p.exists() {
                    println!("Note: a config file already exists at that location.");
                } else {
                    println!("Note: no config file exists there yet. Run without --print-config to create a template.");
                }
            }
            None => {
                println!("Could not determine a default config path for this environment (HOME/config dir not set).");
            }
        }
        return Ok(());
    }

    // Create template config if none exists (before logging init)
    if let Some(path) = ensure_default_config_exists() {
        println!("\nA template aria_move config was written to:\n  {}\n", path.display());
        println!("Edit the file to set download_base, completed_base and optionally log_level and log_file, for example:\n\n<config>\n  <download_base>/path/to/incoming</download_base>\n  <completed_base>/path/to/completed</completed_base>\n  <log_level>normal</log_level>\n  <log_file>/path/to/aria_move.log</log_file>\n</config>\n");
        println!("Then re-run this command. To use a different location set ARIA_MOVE_CONFIG.\n");
        return Ok(());
    }

    // Build config (may read XML). CLI args override config values.
    let mut cfg = Config::default();
    if let Some(db) = args.download_base.as_ref() {
        cfg.download_base = db.clone();
    }
    if let Some(cb) = args.completed_base.as_ref() {
        cfg.completed_base = cb.clone();
    }

    // logging level: parse CLI string into LogLevel, or use --debug as shorthand.
    if let Some(lvl_str) = args.log_level.as_ref() {
        if let Some(parsed) = LogLevel::parse(lvl_str) {
            cfg.log_level = parsed;
        } else {
            eprintln!("Unknown log level '{}', using '{:?}'", lvl_str, cfg.log_level);
        }
    } else if args.debug {
        cfg.log_level = LogLevel::Debug;
    }

    // dry-run propagation
    if args.dry_run {
        cfg.dry_run = true;
        println!("Running in dry-run mode: no filesystem changes will be made.");
    }

    // preserve_metadata propagation
    if args.preserve_metadata {
        cfg.preserve_metadata = true;
    }

    // initialize logging and capture the guard so we can drop it on signal
    let guard_opt = init_logging_level(&cfg.log_level, cfg.log_file.as_deref(), args.json).map_err(|e| {
        eprintln!("Failed to initialize logging: {}", e);
        e
    })?;

    // wrap the guard for safe drop in the ctrlc handler
    let guard_slot = Arc::new(Mutex::new(guard_opt));
    {
        let guard_slot = Arc::clone(&guard_slot);
        ctrlc::set_handler(move || {
            // set shutdown flag first so work can abort
            shutdown::request();
            eprintln!("Received interrupt; shutting down gracefully...");

            // take and drop the WorkerGuard to flush any buffered logs
            if let Ok(mut g) = guard_slot.lock() {
                if g.is_some() {
                    let _ = g.take(); // drop guard here to flush tracing_appender
                }
            }
        }).expect("failed to install signal handler");
    }

    // optional: early abort if signal already requested
    if shutdown::is_requested() {
        return Ok(());
    }

    info!("Starting aria_move: {:?}", args);

    // run main logic in a fallible block so we can flush log guard before returning
    let result = (|| -> Result<()> {
        // Validate paths and permissions before proceeding.
        validate_paths(&cfg)?;

        // Resolve source path (try provided path first, else find recent)
        let maybe_src = args.source_path.as_deref();
        let src = match aria_move::resolve_source_path(&cfg, maybe_src) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to resolve a source path: {:?}", e);
                return Err(e);
            }
        };

        match move_entry(&cfg, &src) {
            Ok(dest) => {
                // Normal-level user-facing success
                info!(source = %src.display(), dest = %dest.display(), "Move completed");
                Ok(())
            }
            Err(e) => {
                // Always log errors regardless of level
                error!(error = ?e, "Move failed");
                Err(e)
            }
        }
    })();

    // Ensure log appender is flushed/shutdown before exit so file logs are complete.
    if let Some(guard) = guard_opt {
        // drop the guard to allow background worker to flush and shut down
        drop(guard);
    }

    result
}
