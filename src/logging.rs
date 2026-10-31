//! Two independent log outputs.
//!
//! The system log (syslog/journald on Unix, the Application event log on
//! Windows, mirrored to stderr) receives every application warning, error and
//! crash, whatever the configuration says, including failures that happen
//! before the configuration is loaded.
//!
//! The log file receives what `log_level` selects:
//! `verbose` everything, `normal` completed moves plus warnings and errors,
//! `warn` warnings and errors only.
use crate::{LogLevel, output, platform};
use anyhow::Result;
use std::backtrace::{Backtrace, BacktraceStatus};
use std::fmt::{self, Write as _};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Metadata, Subscriber};
use tracing_subscriber::fmt::{format::Writer, time::FormatTime};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::{
    Layer,
    filter::{LevelFilter, filter_fn},
    registry::LookupSpan,
    util::SubscriberInitExt,
};

/// Target of the per-move success record kept by `normal` file logs.
pub const COMPLETION: &str = "aria_move::completion";

/// Set once the subscriber is installed and events reach both outputs.
static READY: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

/// Write straight to the system log and stderr, bypassing the log file.
pub fn system(severity: Severity, message: &str) {
    platform::system_log(severity, message);
    match severity {
        Severity::Error => output::print_error(message),
        Severity::Warning => output::print_warn(message),
    }
}

/// Report an application error: always to the system log, and to the log
/// file too once [`init`] has opened it.
pub fn error(message: &str) {
    if READY.load(Ordering::Acquire) {
        tracing::error!("{message}");
    } else {
        system(Severity::Error, message);
    }
}

/// Report crashes through [`error`]. Install before anything else can panic.
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let backtrace = Backtrace::capture();
        if backtrace.status() == BacktraceStatus::Captured {
            error(&format!("Application panicked: {info}\n{backtrace}"));
        } else {
            error(&format!("Application panicked: {info}"));
        }
    }));
}

/// Forwards warnings and errors to [`system`].
struct SystemLayer;

impl<S: Subscriber> Layer<S> for SystemLayer {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let severity = match *event.metadata().level() {
            Level::ERROR => Severity::Error,
            Level::WARN => Severity::Warning,
            _ => return,
        };
        let mut text = Text::default();
        event.record(&mut text);
        system(severity, &text.finish());
    }
}

/// An event as `message key=value ...`.
#[derive(Default)]
struct Text {
    message: String,
    fields: String,
}

impl Text {
    fn finish(self) -> String {
        if self.fields.is_empty() {
            self.message
        } else if self.message.is_empty() {
            self.fields
        } else {
            format!("{} {}", self.message, self.fields)
        }
    }
}

impl Visit for Text {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_debug(field, &format_args!("{value}"));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            if !self.fields.is_empty() {
                self.fields.push(' ');
            }
            let _ = write!(self.fields, "{}={value:?}", field.name());
        }
    }
}

struct LocalTime;

impl FormatTime for LocalTime {
    fn format_time(&self, writer: &mut Writer<'_>) -> fmt::Result {
        write!(
            writer,
            "{}",
            chrono::Local::now().format("%d/%m/%y %H:%M:%S")
        )
    }
}

/// Whether the log file at `level` records an event.
fn file_enabled(level: LogLevel, metadata: &Metadata<'_>) -> bool {
    let problem = matches!(*metadata.level(), Level::ERROR | Level::WARN);
    match level {
        LogLevel::Verbose => true,
        LogLevel::Normal => problem || metadata.target() == COMPLETION,
        LogLevel::Warn => problem,
    }
}

fn file_layer<S>(file: Mutex<std::fs::File>, level: LogLevel, json: bool) -> impl Layer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let layer = tracing_subscriber::fmt::layer()
        .with_timer(LocalTime)
        .with_target(false)
        .with_ansi(false)
        .with_writer(file);
    let layer = if json {
        layer.json().boxed()
    } else {
        layer.compact().boxed()
    };
    layer.with_filter(filter_fn(move |metadata| file_enabled(level, metadata)))
}

/// Install both outputs. A log file that cannot be opened is reported to the
/// system log and skipped; moves continue. File writes are synchronous so
/// nothing queued is lost when the short-lived hook exits or panics.
pub fn init(level: LogLevel, log_file: Option<&Path>, json: bool) -> Result<()> {
    let file = log_file.and_then(|path| match platform::open_log_append(path) {
        Ok(file) => Some(Mutex::new(file)),
        Err(error) => {
            system(
                Severity::Error,
                &format!(
                    "Cannot open log file '{}': {error}. File logging is disabled.",
                    path.display()
                ),
            );
            None
        }
    });
    tracing_subscriber::registry()
        .with(SystemLayer.with_filter(LevelFilter::WARN))
        .with(file.map(|file| file_layer(file, level, json)))
        .try_init()?;
    READY.store(true, Ordering::Release);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_policy_per_level() {
        use tracing::callsite::Identifier;
        use tracing::metadata::Kind;

        struct Site;
        static SITE: Site = Site;
        impl tracing::Callsite for Site {
            fn set_interest(&self, _: tracing::subscriber::Interest) {}
            fn metadata(&self) -> &Metadata<'_> {
                unreachable!()
            }
        }
        let meta = |level, target| {
            Metadata::new(
                "event",
                target,
                level,
                None,
                None,
                None,
                tracing::field::FieldSet::new(&[], Identifier(&SITE)),
                Kind::EVENT,
            )
        };

        // (event level, target) -> recorded at warn, normal, verbose
        let cases = [
            ((Level::ERROR, "aria_move"), [true, true, true]),
            ((Level::WARN, "aria_move"), [true, true, true]),
            ((Level::INFO, COMPLETION), [false, true, true]),
            ((Level::INFO, "aria_move"), [false, false, true]),
            ((Level::DEBUG, "aria_move"), [false, false, true]),
        ];
        for ((level, target), expected) in cases {
            let metadata = meta(level, target);
            let actual = [LogLevel::Warn, LogLevel::Normal, LogLevel::Verbose]
                .map(|file| file_enabled(file, &metadata));
            assert_eq!(actual, expected, "{level} {target}");
        }
    }
}
