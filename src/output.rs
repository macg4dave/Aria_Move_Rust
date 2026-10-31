//! Prefixed, optionally colored terminal messages for humans.
use std::io::IsTerminal;

/// Color policy: `NO_COLOR` disables, `CLICOLOR_FORCE=1` forces, `CLICOLOR=0`
/// disables, otherwise color only when the stream is a terminal.
pub fn color_enabled(terminal: bool) -> bool {
    resolve_color(
        std::env::var_os("NO_COLOR").is_some(),
        std::env::var_os("CLICOLOR_FORCE").is_some_and(|v| v == "1"),
        std::env::var_os("CLICOLOR").is_some_and(|v| v == "0"),
        terminal,
    )
}

fn resolve_color(no_color: bool, force: bool, disabled: bool, terminal: bool) -> bool {
    !no_color && (force || (!disabled && terminal))
}

#[derive(Copy, Clone)]
enum Kind {
    Info,
    Warn,
    Error,
    Ok,
}

fn format_line(kind: Kind, msg: &str, color: bool) -> String {
    // Bold cyan, yellow, red and green ANSI SGR sequences.
    let (prefix, sgr) = match kind {
        Kind::Info => ("info:", "1;36"),
        Kind::Warn => ("warn:", "1;33"),
        Kind::Error => ("error:", "1;31"),
        Kind::Ok => ("ok:", "1;32"),
    };
    if color {
        format!("\x1b[{sgr}m{prefix}\x1b[0m {msg}")
    } else {
        format!("{prefix} {msg}")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_and_colored_prefixes() {
        assert_eq!(format_line(Kind::Info, "hello", false), "info: hello");
        assert_eq!(format_line(Kind::Warn, "careful", false), "warn: careful");
        assert_eq!(format_line(Kind::Error, "boom", false), "error: boom");
        assert_eq!(format_line(Kind::Ok, "done", false), "ok: done");
        assert_eq!(
            format_line(Kind::Error, "boom", true),
            "\x1b[1;31merror:\x1b[0m boom"
        );
    }

    #[test]
    fn color_policy_precedence() {
        // (NO_COLOR, CLICOLOR_FORCE=1, CLICOLOR=0, terminal) -> enabled
        let cases = [
            ((false, false, false, true), true),
            ((false, false, false, false), false),
            ((true, true, false, true), false),
            ((false, true, true, false), true),
            ((false, false, true, true), false),
        ];
        for ((no_color, force, disabled, terminal), expected) in cases {
            assert_eq!(resolve_color(no_color, force, disabled, terminal), expected);
        }
    }
}
