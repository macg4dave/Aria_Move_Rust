//! Tracing initialization.
//! Builds a subscriber with EnvFilter, supports compact or JSON formats, and optional file logging.
//!
//! Behavior:
//! - Log level is driven by LogLevel (no RUST_LOG override here).
//! - JSON/non-JSON stdout formatting is selected via the `json` flag.
//! - If `log_file` is provided and passes safety checks, a non-blocking file layer is added.
//!
//! Implementation notes:
//! - File logging uses tracing_appender::non_blocking to avoid blocking on I/O.
//! - We refuse file logging if any ancestor of the file path is a symlink.

use anyhow::Result;
use aria_move::{path_has_symlink_ancestor, LogLevel, default_log_path};
use aria_move::output as out;
use chrono::Local;
use std::fmt as stdfmt;
use std::path::Path;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::fmt as tsfmt;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry;
use tracing_subscriber::util::SubscriberInitExt;

use aria_move::platform::open_log_file_secure_append;

/// Human-friendly timestamp formatter (DD/MM/YY HH:MM:SS)
struct LocalHumanTime;
impl FormatTime for LocalHumanTime {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> stdfmt::Result {
        let now = Local::now();
        write!(w, "{}", now.format("%d/%m/%y %H:%M:%S"))
    }
}

#[inline]
fn to_level_filter(lvl: &LogLevel) -> LevelFilter {
    match lvl {
        LogLevel::Quiet => LevelFilter::ERROR,
        LogLevel::Normal => LevelFilter::INFO,
        LogLevel::Info => LevelFilter::DEBUG,
        LogLevel::Debug => LevelFilter::TRACE,
    }
}

#[inline]
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

/// Try to open a non-blocking file writer for logging:
/// - Refuse if any ancestor is a symlink (prints a warning and returns None)
/// - Best-effort create parent directory
/// - Open file for append and wrap with non_blocking
fn maybe_open_non_blocking_writer(path: &Path) -> Option<(NonBlocking, WorkerGuard)> {
    match path_has_symlink_ancestor(path) {
        Ok(true) => {
            eprintln!(
                "Refusing to enable file logging: ancestor of {} is a symlink; proceeding without file logging.",
                path.display()
            );
            return None;
        }
        Err(e) => {
            eprintln!(
                "Error checking log path {} for symlinks: {}; proceeding without file logging.",
                path.display(),
                e
            );
            return None;
        }
        Ok(false) => {}
    }

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match open_log_file_secure_append(path) {
        Ok(file) => {
            let (writer, guard) = tracing_appender::non_blocking(file);
            Some((writer, guard))
        }
        Err(e) => {
            eprintln!("Failed to open log file {}: {}", path.display(), e);
            None
        }
    }
}

/// Initialize tracing based on LogLevel and format. Returns an optional WorkerGuard
/// if a file appender is created (must be held until shutdown to flush logs).
pub fn init_tracing(
    lvl: &LogLevel,
    log_file: Option<&Path>,
    json: bool,
) -> Result<Option<WorkerGuard>> {
    let level_filter = to_level_filter(lvl);
    let env_filter = env_filter_from_level(level_filter);

    // Build stdout layer per format and initialize later to avoid type mismatch across branches

    // Optional file layer
    if let Some(path) = log_file {
        if let Some((writer, guard)) = maybe_open_non_blocking_writer(path) {
            if json {
                let stdout_layer = tsfmt::layer()
                    .event_format(tsfmt::format().json())
                    .with_timer(LocalHumanTime)
                    .with_level(true)
                    .with_target(true)
                    .with_thread_ids(true);
                let file_layer = tsfmt::layer()
                    .event_format(tsfmt::format().json())
                    .with_timer(LocalHumanTime)
                    .with_level(true)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_writer(writer);
                registry()
                    .with(env_filter)
                    .with(stdout_layer)
                    .with(file_layer)
                    .init();
            } else {
                let stdout_layer = tsfmt::layer()
                    .with_timer(LocalHumanTime)
                    .with_level(true)
                    .with_target(true)
                    .with_thread_ids(true)
                    .compact();
                let file_layer = tsfmt::layer()
                    .with_timer(LocalHumanTime)
                    .with_level(true)
                    .with_target(true)
                    .with_thread_ids(true)
                    .compact()
                    .with_writer(writer);
                registry()
                    .with(env_filter)
                    .with(stdout_layer)
                    .with(file_layer)
                    .init();
            }
            return Ok(Some(guard));
        }
        // maybe_open_non_blocking_writer already printed a short reason to stderr.
        // Provide a clearer, actionable message to users running the binary so
        // they can diagnose why file logging was not enabled.
        out::print_warn(&format!(
            "Requested file logging to '{}' was not enabled. Check that the parent directory exists, is writable by this process, and that no ancestor is a symlink. Logs will continue to stdout.",
            path.display()
        ));
        if let Ok(def) = default_log_path() {
            out::print_info(&format!("You can try using the default log path instead: {}", def.display()));
        }
    }

    // No file layer (either not requested or refused/failed)
    if json {
        let stdout_layer = tsfmt::layer()
            .event_format(tsfmt::format().json())
            .with_timer(LocalHumanTime)
            .with_level(true)
            .with_target(true)
            .with_thread_ids(true);
        registry().with(env_filter).with(stdout_layer).init();
    } else {
        let stdout_layer = tsfmt::layer()
            .with_timer(LocalHumanTime)
            .with_level(true)
            .with_target(true)
            .with_thread_ids(true)
            .compact();
        registry().with(env_filter).with(stdout_layer).init();
    }
    Ok(None)
}
