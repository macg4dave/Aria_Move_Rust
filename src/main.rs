//! Binary entry: load configuration, set up logging and signals, then move one entry.
use anyhow::{Context, Result, bail};
use aria_move::cli::Args;
use aria_move::{config, errors, logging, output, shutdown};
use clap::Parser;

fn main() {
    if let Err(error) = run(Args::parse()) {
        // Include causes so an actionable OS error is not hidden by context.
        output::print_error(&format!("{error:#}"));
        std::process::exit(1);
    }
}

fn run(args: Args) -> Result<()> {
    let config_path = match &args.config_path {
        Some(path) => config::resolve_config_path(path)?,
        None => config::default_config_path()?,
    };
    if args.print_config {
        output::print_info(&format!("Config path:\n  {}", config_path.display()));
        return Ok(());
    }
    let mut cfg = match std::fs::symlink_metadata(&config_path) {
        Ok(_) => config::load_config(&config_path)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if args.dry_run {
                bail!(
                    "Config missing: {}; a dry run does not create one",
                    config_path.display()
                );
            }
            config::create_template_config(&config_path)?;
            output::print_success(&format!(
                "A template config was written to: {}",
                config_path.display()
            ));
            output::print_info("Edit download_base and completed_base, then run again.");
            return Ok(());
        }
        Err(e) => {
            return Err(e).with_context(|| format!("inspect config {}", config_path.display()));
        }
    };
    args.apply_overrides(&mut cfg);
    // Legacy environment switch, still documented for service files.
    cfg.disable_locks |= std::env::var_os("ARIA_MOVE_DISABLE_LOCKS").is_some_and(|v| v == "1");
    // A dry run logs to a file only when one is configured explicitly.
    let log_file = match &cfg.log_file {
        Some(path) => Some(path.clone()),
        None if !cfg.dry_run => config::default_log_path(&config_path),
        None => None,
    };
    let _guard = logging::init(cfg.log_level, log_file.as_deref(), args.json)?;
    ctrlc::set_handler(shutdown::request).context("install signal handler")?;

    let result = execute(&args, &mut cfg);
    if let Err(error) = &result {
        tracing::error!(code = errors::error_code(error), error = ?error, "Move failed");
    }
    result
}

fn execute(args: &Args, cfg: &mut config::Config) -> Result<()> {
    shutdown::check()?;
    config::validate_and_normalize(cfg)?;
    let src = aria_move::resolve_source_path(cfg, args.resolved_source())?;
    let dest = aria_move::move_entry(cfg, &src)?;
    if cfg.dry_run && !args.json {
        output::print_info(&format!(
            "Dry-run: would move '{}' -> '{}'",
            src.display(),
            dest.display()
        ));
    }
    tracing::info!(source = %src.display(), dest = %dest.display(), dry_run = cfg.dry_run, "Move completed");
    Ok(())
}
