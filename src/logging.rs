//! Tracing setup: one formatter policy for stdout and an optional log file.
use crate::{LogLevel, output, platform};
use anyhow::Result;
use std::io::IsTerminal;
use std::path::Path;
use tracing::Subscriber;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::{self, MakeWriter, format::Writer, time::FormatTime};
use tracing_subscriber::{
    Layer, layer::SubscriberExt, registry::LookupSpan, util::SubscriberInitExt,
};

struct LocalTime;

impl FormatTime for LocalTime {
    fn format_time(&self, writer: &mut Writer<'_>) -> std::fmt::Result {
        write!(
            writer,
            "{}",
            chrono::Local::now().format("%d/%m/%y %H:%M:%S")
        )
    }
}

fn layer<S, W>(writer: W, json: bool, ansi: bool) -> Box<dyn Layer<S> + Send + Sync>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let layer = fmt::layer()
        .with_timer(LocalTime)
        .with_target(false)
        .with_ansi(ansi)
        .with_writer(writer);
    if json {
        layer.json().boxed()
    } else {
        layer.compact().boxed()
    }
}

/// Install the global subscriber. A log file that cannot be opened is reported
/// and skipped; logging to stdout continues. Keep the guard alive until exit
/// so buffered file output is flushed.
pub fn init(level: LogLevel, log_file: Option<&Path>, json: bool) -> Result<Option<WorkerGuard>> {
    let file = log_file.and_then(|path| match platform::open_log_append(path) {
        Ok(file) => Some(tracing_appender::non_blocking(file)),
        Err(error) => {
            output::print_warn(&format!(
                "File logging to '{}' is disabled: {error}. Logs continue on stdout.",
                path.display()
            ));
            None
        }
    });
    let (file_layer, guard) = match file {
        Some((writer, guard)) => (Some(layer(writer, json, false)), Some(guard)),
        None => (None, None),
    };
    let ansi = !json && output::color_enabled(std::io::stdout().is_terminal());
    tracing_subscriber::registry()
        .with(level.filter())
        .with(layer(std::io::stdout, json, ansi))
        .with(file_layer)
        .try_init()?;
    Ok(guard)
}
