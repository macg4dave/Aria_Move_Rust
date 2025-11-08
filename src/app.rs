//! Application orchestrator.
//! Loads/merges config, initializes logging, installs signal handlers, validates paths,
//! resolves the source, and invokes the appropriate move operation.

use anyhow::Result;
use aria_move::AriaMoveError;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info};
use aria_move::output as out;

use aria_move::{
    default_config_path, move_entry, resolve_source_path, shutdown, Config, LogLevel,
};
use aria_move::config::{validate_and_normalize, load_or_init, LoadResult};
use aria_move::config::xml::load_config_from_xml;

use aria_move::cli::Args;
use crate::logging::init_tracing;

/// Run the CLI application.
pub fn run(args: Args) -> Result<()> {
    // Handle --print-config before logging init
    if args.print_config {
        if let Ok(cfg_env) = std::env::var("ARIA_MOVE_CONFIG") {
            out::print_info(&format!("Using ARIA_MOVE_CONFIG (explicit):\n  {}\n", cfg_env));
            out::print_info("To override, unset ARIA_MOVE_CONFIG or set it to another file.");
            return Ok(());
        }
        match default_config_path() {
            Ok(p) => {
                out::print_info(&format!("Default aria_move config path:\n  {}\n", p.display()));
                if p.exists() {
                    out::print_info("A config file already exists at that location.");
                } else {
                    out::print_info("No config file exists there yet. Run without --print-config to create a template.");
                }
            }
            Err(e) => {
                out::print_error(&format!("Could not determine a default config path: {e}"));
            }
        }
        return Ok(());
    }

    // Create template config if none exists (before logging init)
    if let LoadResult::CreatedTemplate(path) = load_or_init()? {
        out::print_success(&format!("A template aria_move config was written to: {}", path.display()));
        out::print_info("Edit the file to set `download_base`, `completed_base` and optionally `log_level` and `log_file`. Example:\n\n<config>\n  <download_base>/path/to/incoming</download_base>\n  <completed_base>/path/to/completed</completed_base>\n  <log_level>normal</log_level>\n  <log_file>/path/to/aria_move.log</log_file>\n</config>\n");
        out::print_info("Then re-run this command. To use a different location set ARIA_MOVE_CONFIG.");
        return Ok(());
    }

    // Build config (may read XML). CLI args override config values.
    let mut cfg = Config::default();

    // Prefer config file values unless CLI overrides them.
    if let Some((db, cb, lvl, lf, recent_window, preserve_metadata, preserve_permissions)) = load_config_from_xml() {
        if args.download_base.is_none() {
            cfg.download_base = db;
        }
        if args.completed_base.is_none() {
            cfg.completed_base = cb;
        }
        if args.log_level.is_none() && let Some(l) = lvl { cfg.log_level = l; }
        if cfg.log_file.is_none() {
            cfg.log_file = lf;
        }
        cfg.recent_window = recent_window;
        cfg.preserve_metadata = preserve_metadata;
        // Only set permissions flag if full metadata not requested (XML semantics mirror CLI precedence)
        if !cfg.preserve_metadata {
            cfg.preserve_permissions = preserve_permissions;
        }
    }

    // Apply CLI overrides (CLI wins)
    if let Some(db) = args.download_base.as_ref() {
        cfg.download_base = db.clone();
    }
    if let Some(cb) = args.completed_base.as_ref() {
        cfg.completed_base = cb.clone();
    }
    if let Some(lvl_str) = args.log_level.as_ref() {
        if let Some(parsed) = LogLevel::parse(lvl_str) {
            cfg.log_level = parsed;
        }
    } else if args.debug {
        cfg.log_level = LogLevel::Debug;
    }
    if args.preserve_metadata {
        cfg.preserve_metadata = true;
    }
    if args.preserve_permissions && !cfg.preserve_metadata { cfg.preserve_permissions = true; }
    if args.dry_run {
        cfg.dry_run = true;
    }

    // Initialize logging and capture the guard so we can drop it on signal
    let guard_opt: Option<tracing_appender::non_blocking::WorkerGuard> =
        init_tracing(&cfg.log_level, cfg.log_file.as_deref(), args.json).map_err(|e| {
            out::print_error(&format!("Failed to initialize logging: {}", e));
            e
        })?;

    // Guard needs to be dropped on SIGINT to flush logs
    let guard_slot = Arc::new(Mutex::new(guard_opt));
    {
        let guard_slot = Arc::clone(&guard_slot);
        ctrlc::set_handler(move || {
            shutdown::request();
            out::print_warn("Received interrupt; shutting down gracefully...");
            if let Ok(mut g) = guard_slot.lock() {
                let _ = g.take(); // drop guard here to flush tracing_appender
            }
        })
        .expect("failed to install signal handler");
    }

    if shutdown::is_requested() {
        return Ok(());
    }

    debug!("Starting aria_move: {:?}", args);

    // Main run (so we can drop guard after)
    let result = (|| -> Result<()> {
        // Ensure required directories exist and canonicalize paths
        validate_and_normalize(&mut cfg)?;
        let maybe_src_owned = args.resolved_source();
        // If user explicitly provided a directory, accept it directly (bypass file-only resolver)
        let src_result: Result<std::path::PathBuf> = if let Some(p) = maybe_src_owned.as_deref() {
            match std::fs::symlink_metadata(p) {
                Ok(meta) if meta.file_type().is_dir() => Ok(p.to_path_buf()),
                _ => resolve_source_path(&cfg, Some(p)),
            }
        } else {
            resolve_source_path(&cfg, None)
        };

        let src = match src_result {
            Ok(p) => p,
            Err(e) => {
                if let Some(am) = e.downcast_ref::<AriaMoveError>() {
                    let code = am.code();
                    match am {
                        AriaMoveError::ProvidedNotFile(path) => {
                            error!(code, kind = "provided_not_file", path = %path.display(), "Source path is not a regular file")
                        }
                        AriaMoveError::Disappeared(path) => {
                            error!(code, kind = "disappeared", path = %path.display(), "Resolved path disappeared before use")
                        }
                        AriaMoveError::NoneFound(base) => {
                            error!(code, kind = "none_found", base = %base.display(), "No candidate file found under base")
                        }
                        AriaMoveError::BaseInvalid(base) => {
                            error!(code, kind = "base_invalid", base = %base.display(), "Download base invalid or not a directory")
                        }
                        _ => {
                            error!(code, kind = "resolve_error", error = ?am, "Failed to resolve a source path")
                        }
                    }
                } else {
                    error!(error = ?e, "Failed to resolve a source path");
                }
                return Err(e);
            }
        };
        match move_entry(&cfg, &src) {
            Ok(dest) => {
                if cfg.dry_run {
                    out::print_info(&format!(
                        "Dry-run: would move '{}' -> '{}'",
                        src.display(),
                        dest.display()
                    ));
                }
                info!(source = %src.display(), dest = %dest.display(), "Move completed");
                Ok(())
            }
            Err(e) => {
                if let Some(am) = e.downcast_ref::<AriaMoveError>() {
                    let code = am.code();
                    match am {
                        AriaMoveError::SourceNotFound(path) => {
                            error!(code, kind = "source_not_found", path = %path.display(), "Move failed")
                        }
                        AriaMoveError::PermissionDenied { path, context } => {
                            error!(code, kind = "permission_denied", path = %path.display(), %context, "Move failed")
                        }
                        AriaMoveError::InsufficientSpace {
                            required,
                            available,
                            dest,
                        } => {
                            error!(code, kind = "insufficient_space", required = *required, available = *available, dest = %dest.display(), "Move failed")
                        }
                        AriaMoveError::Interrupted => {
                            error!(code, kind = "interrupted", "Move aborted by user")
                        }
                        AriaMoveError::ProvidedNotFile(path) => {
                            error!(code, kind = "provided_not_file", path = %path.display(), "Move failed")
                        }
                        AriaMoveError::Disappeared(path) => {
                            error!(code, kind = "disappeared", path = %path.display(), "Move failed")
                        }
                        AriaMoveError::NoneFound(base) => {
                            error!(code, kind = "none_found", base = %base.display(), "Move failed")
                        }
                        AriaMoveError::BaseInvalid(base) => {
                            error!(code, kind = "base_invalid", base = %base.display(), "Move failed")
                        }
                        _ => {
                            error!(code, kind = "move_error", error = ?am, "Move failed")
                        }
                    }
                } else {
                    error!(error = ?e, "Move failed");
                }
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
