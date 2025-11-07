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
use aria_move::output as out;

fn main() -> Result<()> {
    let args = cli::parse();

    // Order: default XML -> ARIA_MOVE_CONFIG -> load_or_init fallback
    let mut cfg = if let Some(cfg) = load_config_from_default_xml()? {
        out::print_info("Using config from default XML path");
        cfg
    } else if let Some(cfg) = load_config_from_xml_env()? {
        out::print_info("Using config from ARIA_MOVE_CONFIG");
        cfg
    } else {
        match config::load_or_init()? {
            LoadResult::Loaded(cfg, _) => cfg,
            LoadResult::CreatedTemplate(path) => {
                out::print_success(&format!("Created template config at: {}", path.display()));
                out::print_info("Edit the file to set `download_base` and `completed_base`, then rerun.");
                std::process::exit(0);
            }
        }
    };

    // Helpful debug summary (still user-friendly). Use print_info so it shows before
    // logging initialization; these are high-level diagnostics.
    out::print_info(&format!(
        "Final config - download_base: {}  completed_base: {}",
        cfg.download_base.display(),
        cfg.completed_base.display()
    ));

    args.apply_overrides(&mut cfg);
    config::validate_and_normalize(&mut cfg)?;

    let src = match args.resolved_source() {
        Some(p) => p,
        None => {
            out::print_error("No source provided. Use SOURCE_PATH or --source-path <PATH>.");
            std::process::exit(2);
        }
    };

    let dest = aria_move::fs_ops::move_file(&cfg, &src)?;
    if cfg.dry_run {
        out::print_info(&format!("Dry-run: would move '{}' -> '{}'", src.display(), dest.display()));
    } else {
        // Primary user output - keep this plain so scripts can parse it reliably.
        out::print_user(&format!("Moved '{}' -> '{}'", src.display(), dest.display()));
    }
    Ok(())
}
