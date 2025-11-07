//! Binary entry for aria_move.
//! Delegates orchestration to `app::run` to keep `main` thin and consistent.

mod app;
mod logging;

use anyhow::Result;

fn main() -> Result<()> {
    let args = aria_move::cli::parse();
    app::run(args)
}
