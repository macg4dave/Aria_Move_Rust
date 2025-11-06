//! Minimal binary entry for aria_move.
//! - Parse CLI
//! - Load or initialize config (first run writes a template and exits)
//! - Apply CLI overrides
//! - Validate/canonicalize paths
//! - Move the provided source and print a concise summary

mod cli;

use anyhow::Result;
use aria_move::config::{self, LoadResult};
use aria_move::{load_config_from_default_xml, load_config_from_xml_env};

fn main() -> Result<()> {
    let args = cli::parse();

    // Order: default XML -> ARIA_MOVE_CONFIG -> load_or_init fallback
    let mut cfg = if let Some(cfg) = load_config_from_default_xml()? {
        eprintln!("[INFO] Using config from default XML path");
        cfg
    } else if let Some(cfg) = load_config_from_xml_env()? {
        eprintln!("[INFO] Using config from ARIA_MOVE_CONFIG");
        cfg
    } else {
        match config::load_or_init()? {
            LoadResult::Loaded(cfg, _) => cfg,
            LoadResult::CreatedTemplate(path) => {
                eprintln!("Created template config at: {}", path.display());
                eprintln!("Edit the file to set download_base and completed_base, then rerun.");
                std::process::exit(0);
            }
        }
    };

    eprintln!(
        "[DEBUG] Final config - download_base: {}\n[DEBUG] Final config - completed_base: {}",
        cfg.download_base.display(),
        cfg.completed_base.display()
    );

    args.apply_overrides(&mut cfg);
    config::validate_and_normalize(&mut cfg)?;

    let src = match args.resolved_source() {
        Some(p) => p.to_path_buf(),
        None => {
            eprintln!("No source provided. Use SOURCE_PATH or --source-path <PATH>.");
            std::process::exit(2);
        }
    };

    let dest = aria_move::fs_ops::move_file(&cfg, &src)?;
    if cfg.dry_run {
        println!("Dry-run: would move '{}' -> '{}'", src.display(), dest.display());
    } else {
        println!("Moved '{}' -> '{}'", src.display(), dest.display());
    }
    Ok(())
}
