//! Binary entry for aria_move.
//! Delegates orchestration to `app::run` and prints concise errors without verbose cause chains.

mod app;
mod logging;

fn main() {
    let args = aria_move::cli::parse();
    if let Err(e) = app::run(args) {
        // Print a single-line, user-friendly error without the default "Caused by" chain.
        // The detailed chain is still available in logs when --debug or JSON logging is enabled.
        aria_move::output::print_error(&format!("{}", e));
        std::process::exit(1);
    }
}
