//! Minimal binary entry for aria_move.
//! - Parse CLI
//! - Load or initialize config (first run writes a template and exits)
//! - Apply CLI overrides
//! - Validate/canonicalize paths
//! - Move the provided source and print a concise summary

mod cli;

use anyhow::Result;
use aria_move::config::{self, LoadResult};

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

    // Apply CLI overrides and validate/canonicalize paths.
    args.apply_overrides(&mut cfg);
    config::validate_and_normalize(&mut cfg)?;

    // Resolve explicit source (positional or --source-path). Fail fast if none provided.
    let src = match args.resolved_source() {
        Some(p) => p.to_path_buf(),
        None => {
            eprintln!("No source provided. Pass SOURCE_PATH (positional) or --source-path <PATH>.");
            std::process::exit(2);
        }
    };

    // Perform the move. move_file returns the actual destination path (may include a deduped suffix).
    match aria_move::fs_ops::move_file(&cfg, &src) {
        Ok(dest) => {
            if cfg.dry_run {
                println!("Dry-run: would move '{}' -> '{}'", src.display(), dest.display());
            } else {
                println!("Moved '{}' -> '{}'", src.display(), dest.display());
            }
            Ok(())
        }
        Err(err) => {
            eprintln!("Error: {err}");
            Err(err)
        }
    }
}
