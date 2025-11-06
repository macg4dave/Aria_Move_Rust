//! Minimal binary entry for aria_move.
//! Parse CLI, load/initialize config, apply overrides, resolve source, run move, and print a summary.

mod cli;

use anyhow::Result;
use aria_move::config::{self, types::Config, LoadResult};
use std::ffi::OsStr;

fn main() -> Result<()> {
    let args = cli::parse();

    // Load or initialize config (first run creates a secure template and exits).
    let mut cfg = match config::load_or_init()? {
        LoadResult::Loaded(cfg, _) => cfg,
        LoadResult::CreatedTemplate(path) => {
            eprintln!("Created template config at: {}", path.display());
            eprintln!("Edit the file to set download_base and completed_base, then rerun.");
            std::process::exit(0);
        }
    };

    // Apply CLI overrides so flags take effect.
    if let Some(p) = &args.download_base {
        cfg.download_base = p.clone();
    }
    if let Some(p) = &args.completed_base {
        cfg.completed_base = p.clone();
    }
    cfg.dry_run = args.dry_run;
    cfg.preserve_metadata = args.preserve_metadata;

    // Validate and normalize paths (create dirs if missing, check perms/symlinks, canonicalize)
    config::validate_and_normalize(&mut cfg)?;

    // Explicit source (positional or --source-path). Fail fast if none provided.
    let src = match args.resolved_source() {
        Some(p) => p.clone(),
        None => {
            eprintln!("No source provided. Pass SOURCE_PATH (positional) or --source-path <PATH>.");
            std::process::exit(2);
        }
    };

    // Compute destination path for user feedback.
    let name = src.file_name().unwrap_or_else(|| OsStr::new("unknown"));
    let dst = cfg.completed_base.join(name);

    if cfg.dry_run {
        println!("Dry-run: would move '{}' -> '{}'", src.display(), dst.display());
    } else {
        println!("Moving '{}' -> '{}'", src.display(), dst.display());
    }

    match aria_move::fs_ops::move_file(&cfg, &src) {
        Ok(_) => {
            println!("{}", if cfg.dry_run { "Dry-run: ok" } else { "Done" });
            Ok(())
        }
        Err(err) => {
            eprintln!("Error: {err}");
            Err(err.into())
        }
    }
}
