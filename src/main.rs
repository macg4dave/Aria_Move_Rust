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
    if args.init {
        let mut cfg = config::Config::default();
        args.apply_overrides(&mut cfg);
        cfg.dry_run = true;
        config::validate_and_normalize(&mut cfg)?;
        config::xml::write_config(&config_path, &cfg)?;
        output::print_success(&format!("Configuration created: {}", config_path.display()));
        return Ok(());
    }
    let mut cfg = match std::fs::symlink_metadata(&config_path) {
        Ok(_) => config::load_config(&config_path)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if args.dry_run || args.check_config {
                bail!(
                    "Config missing: {}; run --init --download-base PATH --completed-base PATH (with the same --config option) to create it",
                    config_path.display()
                );
            }
            config::create_template_config(&config_path)?;
            output::print_success(&format!(
                "A template config was written to: {}",
                config_path.display()
            ));
            bail!(
                "Setup required: edit download_base and completed_base in the template, then run again. Alternatively delete the template and use --init --download-base PATH --completed-base PATH."
            );
        }
        Err(e) => {
            return Err(e).with_context(|| format!("inspect config {}", config_path.display()));
        }
    };
    args.apply_overrides(&mut cfg);
    if args.check_config {
        cfg.dry_run = true;
        config::validate_and_normalize(&mut cfg)?;
        output::print_success(
            "Configuration is valid (directory write access is checked when moving).",
        );
        return Ok(());
    }
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
    let src = match (&args.source_path, &args.source_path_pos, args.num_files) {
        (None, Some(path), Some(count)) => aria_move::resolve_aria2_source(cfg, path, count)?,
        _ => aria_move::resolve_source_path(cfg, args.resolved_source())?,
    };
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
