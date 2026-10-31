//! Binary entry for aria_move.
//! Delegates orchestration to `app::run` and prints contextual terminal failures.

mod app;
mod logging;

fn main() {
    let args = aria_move::cli::parse();
    if let Err(e) = app::run(args) {
        // Include causes on the terminal too, so operation context does not hide
        // an actionable OS error. Structured logs retain the full diagnostic.
        aria_move::output::print_error(&format!("{e:#}"));
        std::process::exit(1);
    }
}
