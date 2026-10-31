//! Structured tracing with one formatter policy for stdout and optional file output.
use anyhow::Result;
use aria_move::{LogLevel, output as out, platform::open_log_file_secure_append};
use chrono::Local;
use std::path::Path;
use tracing::Subscriber;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    Layer,
    filter::EnvFilter,
    fmt::{self, MakeWriter, format::Writer, time::FormatTime},
    layer::SubscriberExt,
    registry::LookupSpan,
    util::SubscriberInitExt,
};

struct LocalHumanTime;
impl FormatTime for LocalHumanTime {
    fn format_time(&self, writer: &mut Writer<'_>) -> std::fmt::Result {
        write!(writer, "{}", Local::now().format("%d/%m/%y %H:%M:%S"))
    }
}

fn formatting_layer<S, W>(writer: W, json: bool, ansi: bool) -> Box<dyn Layer<S> + Send + Sync>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let layer = fmt::layer()
        .with_timer(LocalHumanTime)
        .with_target(false)
        .with_thread_ids(false)
        .with_ansi(ansi)
        .with_writer(writer);
    if json {
        layer.json().boxed()
    } else {
        layer.compact().boxed()
    }
}

pub fn init_tracing(
    level: &LogLevel,
    log_file: Option<&Path>,
    json: bool,
) -> Result<Option<WorkerGuard>> {
    let filter = match level {
        LogLevel::Quiet => "error",
        LogLevel::Normal => "info",
        LogLevel::Info => "debug",
        LogLevel::Debug => "trace",
    };
    let file = log_file.and_then(|path| match open_log_file_secure_append(path) {
        Ok(file) => Some(tracing_appender::non_blocking(file)),
        Err(error) => {
            out::print_warn(&format!(
                "Requested file logging to '{}' was not enabled: {error}. Logs continue to stdout.",
                path.display()
            ));
            None
        }
    });
    let (writer, guard) = match file {
        Some((writer, guard)) => (Some(writer), Some(guard)),
        None => (None, None),
    };
    use std::io::IsTerminal;
    let stdout = formatting_layer(
        std::io::stdout,
        json,
        !json && out::color_enabled(std::io::stdout().is_terminal()),
    );
    let file_layer = writer.map(|writer| formatting_layer(writer, json, false));
    tracing_subscriber::registry()
        .with(EnvFilter::new(filter))
        .with(stdout)
        .with(file_layer)
        .try_init()?;
    Ok(guard)
}
