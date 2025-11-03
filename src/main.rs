use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber::fmt::time::LocalTime;

use aria_move::{Config, move_entry};

/// CLI wrapper for aria_move library
#[derive(Parser, Debug)]
#[command(author, version, about = "Move completed aria2 downloads safely (Rust)")]
struct Args {
    /// Aria2 task id (optional, informational)
    task_id: Option<String>,

    /// Number of files reported by aria2 (0 = unknown)
    num_files: Option<usize>,

    /// Source path passed by aria2
    source_path: Option<PathBuf>,

    /// Optional: override download base (for testing)
    #[arg(long)]
    download_base: Option<PathBuf>,

    /// Optional: override completed base (for testing)
    #[arg(long)]
    completed_base: Option<PathBuf>,
}

fn init_logging() {
    let timer = LocalTime::rfc_3339();
    tracing_subscriber::fmt()
        .with_timer(timer)
        .with_target(false)
        .compact()
        .init();
}

fn main() -> Result<()> {
    init_logging();

    let args = Args::parse();
    info!("Starting aria_move: {:?}", args);

    let mut cfg = Config::default();
    if let Some(db) = args.download_base {
        cfg.download_base = db;
    }
    if let Some(cb) = args.completed_base {
        cfg.completed_base = cb;
    }

    // Resolve source path (try provided path first, else find recent)
    let maybe_src = args.source_path.as_deref();
    let src = match aria_move::resolve_source_path(&cfg, maybe_src) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to resolve a source path: {:?}", e);
            return Err(e.into());
        }
    };

    match move_entry(&cfg, &src) {
        Ok(dest) => {
            info!(source=%src.display(), dest=%dest.display(), "Move completed");
            Ok(())
        }
        Err(e) => {
            error!(error = ?e, "Move failed");
            Err(e)
        }
    }
}
