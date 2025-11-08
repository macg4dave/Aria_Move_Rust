// use macro form directly; no import needed
use std::process::Command;

#[test]
fn binary_print_config_succeeds() {
    let me = assert_cmd::cargo::cargo_bin!("aria_move");
    let out = Command::new(me)
        .arg("--print-config")
        .output()
        .expect("spawn binary");
    assert!(out.status.success(), "binary should succeed with --print-config");
}
