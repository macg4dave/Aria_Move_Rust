use anyhow::Result;
use std::sync::{Arc, Mutex};
use tracing::{error, info};
use ctrlc;

use aria_move::{
    Config,
    LogLevel,
    default_config_path,
    ensure_default_config_exists,
    load_config_from_xml,
    move_entry,
    resolve_source_path,
    shutdown,
    validate_paths,
};

use crate::cli::Args;
use crate::logging::init_tracing;

/// Run the CLI application.
pub fn run(args: Args) -> Result<()> {
    // Handle --print-config before logging init
    if args.print_config {
        if let Ok(cfg_env) = std::env::var("ARIA_MOVE_CONFIG") {
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
            None => println!("Could not determine a default config path for this environment (HOME/config dir not set)."),
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

    // Prefer config file values unless CLI overrides them.
    if let Some((db, cb, lvl, lf, recent_window, preserve_metadata)) = load_config_from_xml() {
        if args.download_base.is_none() { cfg.download_base = db; }
        if args.completed_base.is_none() { cfg.completed_base = cb; }
        if args.log_level.is_none() {
            if let Some(l) = lvl { cfg.log_level = l; }
        }
        if cfg.log_file.is_none() {
            cfg.log_file = lf;
        }
        cfg.recent_window = recent_window;
        cfg.preserve_metadata = preserve_metadata;
    }

    // Apply CLI overrides (CLI wins)
    if let Some(db) = args.download_base.as_ref() { cfg.download_base = db.clone(); }
    if let Some(cb) = args.completed_base.as_ref() { cfg.completed_base = cb.clone(); }
    if let Some(lvl_str) = args.log_level.as_ref() {
        if let Some(parsed) = LogLevel::parse(lvl_str) { cfg.log_level = parsed; }
    } else if args.debug {
        cfg.log_level = LogLevel::Debug;
    }
    if args.preserve_metadata { cfg.preserve_metadata = true; }
    if args.dry_run { cfg.dry_run = true; }

    // Initialize logging and capture the guard so we can drop it on signal
    let guard_opt = init_tracing(&cfg.log_level, cfg.log_file.as_deref(), args.json).map_err(|e| {
        eprintln!("Failed to initialize logging: {}", e);
        e
    })?;

    // Guard needs to be dropped on SIGINT to flush logs
    let guard_slot = Arc::new(Mutex::new(guard_opt));
    {
        let guard_slot = Arc::clone(&guard_slot);
        ctrlc::set_handler(move || {
            shutdown::request();
            eprintln!("Received interrupt; shutting down gracefully...");
            if let Ok(mut g) = guard_slot.lock() {
                let _ = g.take(); // drop guard here to flush tracing_appender
            }
        }).expect("failed to install signal handler");
    }

    if shutdown::is_requested() {
        return Ok(());
    }

    info!("Starting aria_move: {:?}", args);

    // Main run (so we can drop guard after)
    let result = (|| -> Result<()> {
        validate_paths(&cfg)?;
        let maybe_src = args.source_path.as_deref();
        let src = match resolve_source_path(&cfg, maybe_src) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to resolve a source path: {:?}", e);
                return Err(e);
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
    })();

    // Ensure logs are flushed before exit
    if let Ok(mut g) = guard_slot.lock() {
        let _ = g.take();
    }

    result
}