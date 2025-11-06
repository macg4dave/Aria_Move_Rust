//! Minimal binary entry for aria_move.
//! Parse CLI, apply overrides to Config, resolve source, run move, and always print a summary.

mod cli;

use anyhow::Result;
use aria_move::config::types::Config;
use std::ffi::OsStr;

fn main() -> Result<()> {
    let args = cli::parse();

    // Start from defaults, then apply CLI overrides so flags take effect.
    let mut cfg = Config::default();
    if let Some(p) = &args.download_base {
        cfg.download_base = p.clone();
    }
    if let Some(p) = &args.completed_base {
        cfg.completed_base = p.clone();
    }
    cfg.dry_run = args.dry_run;
    cfg.preserve_metadata = args.preserve_metadata;

    // Explicit source (positional or --source-path). Fail fast if none provided.
    let src = match args.resolved_source() {
        Some(p) => p.clone(),
        None => {
            eprintln!("No source provided. Pass SOURCE_PATH (positional) or --source-path <PATH>.");
            std::process::exit(2);
        }
    };

    // Compute destination path for user feedback.
    let name = src
        .file_name()
        .unwrap_or_else(|| OsStr::new("unknown"));
    let dst = cfg.completed_base.join(name);

    if cfg.dry_run {
        println!("Dry-run: would move '{}' -> '{}'", src.display(), dst.display());
    } else {
        println!("Moving '{}' -> '{}'", src.display(), dst.display());
    }

    // Delegate to the library move routine (respects cfg.dry_run internally).
    match aria_move::fs_ops::move_file(&cfg, &src) {
        Ok(_) => {
            if cfg.dry_run {
                println!("Dry-run: no changes made.");
            } else {
                println!("Done.");
            }
            Ok(())
        }
        Err(err) => {
            eprintln!("Error: {}", err);
            Err(err.into())
        }
    }
}
