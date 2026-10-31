use owo_colors::OwoColorize;
use std::env;
use std::io::IsTerminal;

#[inline]
/// Resolve color environment policy for a stream with the supplied terminal status.
pub fn color_enabled(terminal: bool) -> bool {
    // Respect common env conventions first
    if env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if let Ok(v) = env::var("CLICOLOR_FORCE")
        && v == "1"
    {
        return true;
    }
    if let Ok(v) = env::var("CLICOLOR")
        && v == "0"
    {
        return false;
    }
    terminal
}

#[derive(Copy, Clone)]
enum Kind {
    Info,
    Warn,
    Error,
    Ok,
}

#[inline]
fn format_line(kind: Kind, msg: &str, color: bool) -> String {
    match (kind, color) {
        (Kind::Info, true) => format!("{} {}", "info:".cyan().bold(), msg),
        (Kind::Warn, true) => format!("{} {}", "warn:".yellow().bold(), msg),
        (Kind::Error, true) => format!("{} {}", "error:".red().bold(), msg),
        (Kind::Ok, true) => format!("{} {}", "ok:".green().bold(), msg),
        (Kind::Info, false) => format!("info: {}", msg),
        (Kind::Warn, false) => format!("warn: {}", msg),
        (Kind::Error, false) => format!("error: {}", msg),
        (Kind::Ok, false) => format!("ok: {}", msg),
    }
}

pub fn print_info(msg: &str) {
    println!(
        "{}",
        format_line(
            Kind::Info,
            msg,
            color_enabled(std::io::stdout().is_terminal())
        )
    );
}

pub fn print_warn(msg: &str) {
    eprintln!(
        "{}",
        format_line(
            Kind::Warn,
            msg,
            color_enabled(std::io::stderr().is_terminal())
        )
    );
}

pub fn print_error(msg: &str) {
    eprintln!(
        "{}",
        format_line(
            Kind::Error,
            msg,
            color_enabled(std::io::stderr().is_terminal())
        )
    );
}

pub fn print_success(msg: &str) {
    println!(
        "{}",
        format_line(
            Kind::Ok,
            msg,
            color_enabled(std::io::stdout().is_terminal())
        )
    );
}

/// Print a plain user-facing line (no prefix). Use this for primary outputs
/// such as "Moved X -> Y" which users may script against.
pub fn print_user(msg: &str) {
    println!("{}", msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_without_color() {
        assert_eq!(format_line(Kind::Info, "hello", false), "info: hello");
        assert_eq!(
            format_line(Kind::Warn, "be careful", false),
            "warn: be careful"
        );
        assert_eq!(format_line(Kind::Error, "boom", false), "error: boom");
        assert_eq!(format_line(Kind::Ok, "done", false), "ok: done");
    }

    #[test]
    fn formats_with_color_prefix() {
        // We don't hardcode escape sequences; just ensure coloring changes output
        let plain = format_line(Kind::Info, "hello", false);
        let colored = format_line(Kind::Info, "hello", true);
        assert_ne!(plain, colored);
        assert!(colored.contains("hello"));
        assert!(colored.contains("info:"));
    }
}
