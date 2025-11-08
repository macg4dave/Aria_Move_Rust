use aria_move::cli::Args;
use aria_move::config::types::{Config, LogLevel};
use clap::Parser;
use std::path::PathBuf;

#[test]
fn resolved_source_precedence_flag_over_positional() {
    let args = Args::parse_from([
        "aria_move",
        "--source-path",
        "/tmp/flag_path",
        "/tmp/pos_path",
    ]);
    let src = args.resolved_source().unwrap();
    assert_eq!(src, PathBuf::from("/tmp/flag_path"));
}

#[test]
fn resolved_source_uses_positional_when_flag_absent() {
    let args = Args::parse_from(["aria_move", "/tmp/pos_path"]);
    let src = args.resolved_source().unwrap();
    assert_eq!(src, PathBuf::from("/tmp/pos_path"));
}

#[test]
fn resolved_source_legacy_heuristic_from_task_id() {
    // No num_files provided; task_id looks like a path -> accept
    let args = Args::parse_from(["aria_move", "file.iso"]);
    let src = args.resolved_source().unwrap();
    assert_eq!(src, PathBuf::from("file.iso"));
}

#[test]
fn effective_log_level_precedence() {
    let args = Args::parse_from(["aria_move", "--debug", "--log-level", "quiet"]);
    let lvl = args.effective_log_level().unwrap();
    assert_eq!(lvl, LogLevel::Debug); // --debug wins

    let args = Args::parse_from(["aria_move", "--log-level", "info"]);
    let lvl = args.effective_log_level().unwrap();
    assert_eq!(lvl, LogLevel::Info);
}

#[test]
fn apply_overrides_sets_flags() {
    let args = Args::parse_from([
        "aria_move",
        "--download-base",
        "/db",
        "--completed-base",
        "/cb",
        "--log-level",
        "info",
        "--dry-run",
        "--preserve-metadata",
    ]);
    let mut cfg = Config::default();
    args.apply_overrides(&mut cfg);
    assert_eq!(cfg.download_base, PathBuf::from("/db"));
    assert_eq!(cfg.completed_base, PathBuf::from("/cb"));
    assert_eq!(cfg.log_level, LogLevel::Info);
    assert!(cfg.dry_run);
    assert!(cfg.preserve_metadata);
}
