use owo_colors::OwoColorize;

/// Small wrapper around stdout/stderr printing to provide consistent, colored
/// user-facing messages. Colors are enabled only when output is a TTY.
fn is_tty() -> bool {
    atty::is(atty::Stream::Stdout)
}

pub fn print_info(msg: &str) {
    if is_tty() {
        println!("{} {}", "info:".cyan().bold(), msg);
    } else {
        println!("info: {}", msg);
    }
}

pub fn print_warn(msg: &str) {
    if is_tty() {
        eprintln!("{} {}", "warn:".yellow().bold(), msg);
    } else {
        eprintln!("warn: {}", msg);
    }
}

pub fn print_error(msg: &str) {
    if is_tty() {
        eprintln!("{} {}", "error:".red().bold(), msg);
    } else {
        eprintln!("error: {}", msg);
    }
}

pub fn print_success(msg: &str) {
    if is_tty() {
        println!("{} {}", "ok:".green().bold(), msg);
    } else {
        println!("ok: {}", msg);
    }
}

/// Print a plain user-facing line (no prefix). Use this for primary outputs
/// such as "Moved X -> Y" which users may script against.
pub fn print_user(msg: &str) {
    println!("{}", msg);
}
