use aria_move::config::types::Config;

#[test]
fn config_defaults_are_sane() {
    let cfg = Config::default();

    // Default log level
    assert_eq!(format!("{}", cfg.log_level), "normal");

    // Default flags
    assert!(!cfg.dry_run, "dry_run should default to false");
    assert!(
        !cfg.preserve_metadata,
        "preserve_metadata should default to false"
    );

    // Auto-pick window removed; explicit source required. (No assertion here.)

    // Default log_file should exist as a path value (we don't assert existence)
    let lf = cfg
        .log_file
        .as_ref()
        .expect("default log_file should be Some");
    assert_eq!(lf.file_name().unwrap().to_string_lossy(), "aria_move.log");
}
