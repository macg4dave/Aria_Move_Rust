//! Tracing initialization.
//! Builds a subscriber with EnvFilter, supports compact or JSON formats, and optional file logging.

use anyhow::Result;
use aria_move::{path_has_symlink_ancestor, LogLevel};
use chrono::Local;
use std::fmt as stdfmt;
use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry;
use tracing_subscriber::util::SubscriberInitExt;

#[cfg(unix)]
use libc;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

/// Human-friendly timestamp formatter (DD/MM/YY HH:MM:SS)
struct LocalHumanTime;
impl FormatTime for LocalHumanTime {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> stdfmt::Result {
        let now = Local::now();
        write!(w, "{}", now.format("%d/%m/%y %H:%M:%S"))
    }
}

fn to_level_filter(lvl: &LogLevel) -> LevelFilter {
    match lvl {
        LogLevel::Quiet => LevelFilter::ERROR,
        LogLevel::Normal => LevelFilter::INFO,
        LogLevel::Info => LevelFilter::DEBUG,
        LogLevel::Debug => LevelFilter::TRACE,
    }
}

fn env_filter_from_level(level_filter: LevelFilter) -> EnvFilter {
    let level_str = match level_filter {
        LevelFilter::ERROR => "error",
        LevelFilter::WARN => "warn",
        LevelFilter::INFO => "info",
        LevelFilter::DEBUG => "debug",
        LevelFilter::TRACE => "trace",
        _ => "info",
    };
    EnvFilter::new(level_str)
}

/// Initialize tracing based on LogLevel and format. Returns an optional WorkerGuard
/// if a file appender is created (must be held until shutdown to flush logs).
pub fn init_tracing(
    lvl: &LogLevel,
    log_file: Option<&Path>,
    json: bool,
) -> Result<Option<WorkerGuard>> {
    use tracing_subscriber::fmt as tsfmt;

    let level_filter = to_level_filter(lvl);
    let env_filter = env_filter_from_level(level_filter);

    if json {
        let stdout_layer = tsfmt::layer()
            .event_format(tsfmt::format().json())
            .with_timer(LocalHumanTime)
            .with_level(true)
            .with_target(false);

        if let Some(path) = log_file {
            match path_has_symlink_ancestor(path) {
                Ok(true) => {
                    eprintln!("Refusing to enable file logging: ancestor of {} is a symlink; proceeding without file logging.", path.display());
                    registry().with(env_filter).with(stdout_layer).init();
                    return Ok(None);
                }
                Err(e) => {
                    eprintln!("Error checking log path {} for symlinks: {}; proceeding without file logging.", path.display(), e);
                    registry().with(env_filter).with(stdout_layer).init();
                    return Ok(None);
                }
                Ok(false) => {}
            }

            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }

            #[cfg(unix)]
            let file = {
                let mut opts = std::fs::OpenOptions::new();
                opts.create(true).append(true);
                opts.mode(0o600);
                opts.custom_flags(libc::O_NOFOLLOW);
                opts.open(path).map_err(|e| {
                    anyhow::anyhow!("Failed to open log file {}: {}", path.display(), e)
                })?
            };
            #[cfg(not(unix))]
            let file = {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .map_err(|e| {
                        anyhow::anyhow!("Failed to open log file {}: {}", path.display(), e)
                    })?
            };

            let (writer, guard) = tracing_appender::non_blocking(file);
            let file_layer = tsfmt::layer()
                .event_format(tsfmt::format().json())
                .with_timer(LocalHumanTime)
                .with_level(true)
                .with_target(false)
                .with_writer(writer);

            registry()
                .with(env_filter)
                .with(stdout_layer)
                .with(file_layer)
                .init();
            Ok(Some(guard))
        } else {
            registry().with(env_filter).with(stdout_layer).init();
            Ok(None)
        }
    } else {
        let stdout_layer = tsfmt::layer()
            .with_timer(LocalHumanTime)
            .with_level(true)
            .with_target(false)
            .compact();

        if let Some(path) = log_file {
            match path_has_symlink_ancestor(path) {
                Ok(true) => {
                    eprintln!("Refusing to enable file logging: ancestor of {} is a symlink; proceeding without file logging.", path.display());
                    registry().with(env_filter).with(stdout_layer).init();
                    return Ok(None);
                }
                Err(e) => {
                    eprintln!("Error checking log path {} for symlinks: {}; proceeding without file logging.", path.display(), e);
                    registry().with(env_filter).with(stdout_layer).init();
                    return Ok(None);
                }
                Ok(false) => {}
            }

            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }

            #[cfg(unix)]
            let file = {
                let mut opts = std::fs::OpenOptions::new();
                opts.create(true).append(true);
                opts.mode(0o600);
                opts.custom_flags(libc::O_NOFOLLOW);
                opts.open(path).map_err(|e| {
                    anyhow::anyhow!("Failed to open log file {}: {}", path.display(), e)
                })?
            };
            #[cfg(not(unix))]
            let file = {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .map_err(|e| {
                        anyhow::anyhow!("Failed to open log file {}: {}", path.display(), e)
                    })?
            };

            let (writer, guard) = tracing_appender::non_blocking(file);
            let file_layer = tsfmt::layer()
                .with_timer(LocalHumanTime)
                .with_level(true)
                .with_target(false)
                .compact()
                .with_writer(writer);

            registry()
                .with(env_filter)
                .with(stdout_layer)
                .with(file_layer)
                .init();
            Ok(Some(guard))
        } else {
            registry().with(env_filter).with(stdout_layer).init();
            Ok(None)
        }
    }
}
