//! CLI orchestration; configuration and operations own their policies.
use anyhow::{Context, Result};
use aria_move::{Config, cli::Args, config, move_entry, output as out, shutdown};
use tracing::{error, info};

pub fn run(args: Args) -> Result<()> {
    let config_path = match args.config_path.as_deref() {
        Some(path) => config::paths::resolve_config_path(path)?,
        None => config::default_config_path()?,
    };
    if args.print_config {
        out::print_info(&format!("Config path:\n  {}", config_path.display()));
        return Ok(());
    }
    let loaded = match std::fs::symlink_metadata(&config_path) {
        Ok(_) => config::xml::load_for_app(&config_path)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if args.dry_run {
                anyhow::bail!(
                    "Config missing: {}; dry-run will not create a template",
                    config_path.display()
                );
            }
            config::xml::create_template_config(&config_path)?;
            out::print_success(&format!(
                "A template aria_move config was written to: {}",
                config_path.display()
            ));
            out::print_info("Edit download_base and completed_base, then re-run this command.");
            return Ok(());
        }
        Err(e) => return Err(e).context("read config path"),
    };
    let mut cfg = loaded.config;
    if let Some(parent) = config_path.parent()
        && !(cfg!(unix) && parent.starts_with("/etc"))
    {
        cfg.log_file = loaded
            .explicit_log
            .clone()
            .or_else(|| Some(parent.join("aria_move.log")));
    }
    args.apply_overrides(&mut cfg);
    cfg.disable_locks |= std::env::var_os("ARIA_MOVE_DISABLE_LOCKS").is_some_and(|v| v == "1");
    // A dry run permits only an explicitly configured log, never a default log.
    let log_path = if cfg.dry_run {
        loaded.explicit_log
    } else {
        cfg.log_file.clone()
    };
    let _guard = crate::logging::init_tracing(&cfg.log_level, log_path.as_deref(), args.json)?;
    ctrlc::set_handler(shutdown::request).context("install signal handler")?;
    let result = execute(&args, &mut cfg);
    if let Err(e) = &result {
        let code = e
            .downcast_ref::<aria_move::AriaMoveError>()
            .map_or("io_error", |e| e.code());
        error!(code, error = ?e, "Move failed");
    }
    result
}

fn execute(args: &Args, cfg: &mut Config) -> Result<()> {
    shutdown::check()?;
    config::validate_and_normalize(cfg)?;
    let source = args.resolved_source();
    let src = aria_move::fs_ops::resolve_download_source(cfg, source.as_deref())?;
    let dest = move_entry(cfg, &src)?;
    if cfg.dry_run && !args.json {
        out::print_info(&format!(
            "Dry-run: would move '{}' -> '{}'",
            src.display(),
            dest.display()
        ));
    }
    info!(source = %src.display(), dest = %dest.display(), dry_run = cfg.dry_run, "Move completed");
    Ok(())
}
