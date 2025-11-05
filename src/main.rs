use anyhow::Result;

mod cli;
mod logging;
mod app;

fn main() -> Result<()> {
    let args = cli::parse();
    app::run(args)
}
