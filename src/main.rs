//! Binary entry point for aria_move.
//! Parses CLI arguments and delegates execution to app::run.

use anyhow::Result;
mod app;
mod cli;
mod logging;

fn main() -> Result<()> {
    let args = cli::parse();
    app::run(args)
}
