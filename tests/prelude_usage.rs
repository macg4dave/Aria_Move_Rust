use aria_move::prelude::*;

#[test]
fn prelude_exports_expected_items() {
    // Ensure types are accessible
    let _cfg = Config::default();
    // Just confirm we can reference a variant and function names.
    let _ = LogLevel::Debug;
    let _err = Error::Interrupted;
    // Functions compile; we won't invoke them (would need real paths)
    // Use type inference to ensure signatures are visible.
    let _resolve_fn: fn(&Config, Option<&std::path::Path>) -> AMResult<std::path::PathBuf> = resolve_source_path;
    let _move_fn: fn(&Config, &std::path::Path) -> AMResult<std::path::PathBuf> = move_entry;
    // Helpers re-exported in prelude
    let _ = default_config_path();
    let _shutdown_fn: fn() = request_shutdown;
    // Placeholder to keep bindings referenced without needless mutations
}
