use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use aria_move::platform::open_log_file_secure_append;
use std::path::PathBuf;
use tempfile::tempdir;
use tracing::info;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{fmt as tsfmt, registry};

/// A simple writer that appends written bytes into an in-memory Vec<u8>.
/// We wrap the Vec in an Arc<Mutex<...>> so the MakeWriter closure can clone it.
#[derive(Clone)]
struct BufferWriter(Arc<Mutex<Vec<u8>>>);

impl Write for BufferWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut guard = self.0.lock().unwrap();
        guard.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn scoped_logging_writes_to_buffer_without_global_side_effects() {
    // Shared in-memory buffer for captured logs
    let buf = Arc::new(Mutex::new(Vec::new()));

    // MakeWriter closure: each call returns a fresh BufferWriter that clones the Arc
    let make_writer = {
        let buf = buf.clone();
        move || BufferWriter(buf.clone())
    };

    // Build a compact formatter layer that writes into our buffer.
    let layer = tsfmt::layer()
        .with_writer(make_writer)
        .with_target(false)
        .compact();

    // Use an EnvFilter to set the level (match library behavior for test)
    let env_filter = EnvFilter::new("info");

    // Construct a subscriber but don't call `.init()` to avoid setting a global.
    let subscriber = registry().with(env_filter).with(layer);

    // Convert into a Dispatch and run scoped with dispatcher::with_default so the
    // test does not change the global subscriber for other tests or end-users.
    let dispatch = tracing::Dispatch::new(subscriber);
    tracing::dispatcher::with_default(&dispatch, || {
        // Emit a couple of log events at different levels
        info!(target: "test_target", "integration-test: hello {}", "world");
    });

    // Read captured output and assert contents
    let contents = {
        let guard = buf.lock().unwrap();
        String::from_utf8_lossy(&guard[..]).to_string()
    };

    // Should contain our message and the target (we disabled target in the layer,
    // but the compact formatter still includes the message text). We check the
    // message text to ensure the I/O path worked.
    assert!(
        contents.contains("integration-test: hello world"),
        "logged output did not contain expected text; contents={}",
        contents
    );
}

#[test]
fn file_logging_writes_to_custom_path_and_verifies_output() {
    // Create a temp directory and a custom log file path inside it
    let td = tempdir().expect("tempdir");
    let log_path: PathBuf = td.path().join("aria_move_test.log");

    // If the tempdir has a symlink ancestor (common on macOS test environments),
    // the production logger would refuse file logging. In that case we skip this
    // test to avoid false failures in CI/dev setups.
    if aria_move::path_has_symlink_ancestor(&log_path).unwrap() {
        eprintln!(
            "Skipping file logging test: path has symlink ancestor: {}",
            log_path.display()
        );
        return;
    }

    // Open a secure append file using the platform helper (same as production)
    let file = open_log_file_secure_append(&log_path).expect("open_log_file_secure_append");

    // Wrap in non-blocking appender used by tracing
    let (writer, guard): (tracing_appender::non_blocking::NonBlocking, WorkerGuard) =
        tracing_appender::non_blocking(file);

    // Build a simple file-only logging layer
    let file_layer = tsfmt::layer()
        .with_writer(move || writer.clone())
        .with_target(false)
        .compact();

    let env_filter = EnvFilter::new("info");

    let subscriber = registry().with(env_filter).with(file_layer);
    let dispatch = tracing::Dispatch::new(subscriber);

    tracing::dispatcher::with_default(&dispatch, || {
        tracing::info!("file-logging-test: written");
    });

    // Drop the guard to flush the non-blocking worker
    drop(guard);

    // Read the file and assert it contains the expected message
    let contents = std::fs::read_to_string(&log_path).expect("read log file");
    assert!(
        contents.contains("file-logging-test"),
        "log file did not contain expected text; contents={}",
        contents
    );
}
