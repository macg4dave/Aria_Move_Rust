# Measured validation baseline

2026-10-31; base revision `fa610d173f7701b76ce20143575ed22170c2243c`.
Windows, PowerShell, host/installed target `x86_64-pc-windows-msvc` only.
rustc 1.99.1 (58cf8f9c3 2026-10-13), Cargo 1.99.1
(5f94df478 2026-08-27); stable toolchain with rustfmt/clippy. No declared MSRV.
Existing user deletions under .github/.vscode and untracked agent/rules/roadmap
files were present before this task and preserved/updated as requested.

Commands ran against unchanged Rust source, Cargo.toml and dependency resolution.
The only Cargo config edit removes a stale CI comment; its behavior is unchanged.
Logs are checked in under [validation/](validation/) as baseline evidence, not
new failures caused by documentation/workflow removal. No application fixes were
attempted. The full gate is currently **not green**.

| Command actually run | Result | Evidence |
|---|---|---|
| `cargo fmt --all -- --check` | Exit 1, existing formatting drift | [fmt.log](validation/fmt.log) |
| `cargo clippy --all-targets -- -D warnings` | Exit 101, eight library diagnostics | [clippy.log](validation/clippy.log) |
| Same clippy command with command-local empty RUSTFLAGS and CARGO_ENCODED_RUSTFLAGS | Exit 101, same eight diagnostics | [strict.log](validation/strict.log) |
| `cargo test --all --no-fail-fast` | Exit 101; 113 passed, 2 failed, 0 ignored, including one passing doctest | [test.log](validation/test.log) |
| `cargo build` | Exit 0; four library warnings | [build.log](validation/build.log) |
| `cargo test --all --features test-helpers --no-fail-fast` | Exit 101; 113 passed, 2 failed, same targets | [features.log](validation/features.log) |
| `cargo deny check --config deny.toml --hide-inclusion-graph` | Exit 2; installed CLI requires config before subcommand | Corrected command below |
| `cargo deny --config deny.toml check --hide-inclusion-graph` | Exit 1; configuration deserialization fails: unmaintained/unsound `warn` invalid in cargo-deny 0.20.2 | [deny-corrected.log](validation/deny-corrected.log) |
| `cargo audit --version` | Command unavailable | No advisory audit performed |

There are 66 integration source files and 69 test-result summaries (library,
binary, integration executables and doctest). Zero-test cfg-filtered executables
are not native Unix coverage. Counts were summed from the actual result summaries;
no previously documented count was assumed. Discovery identity changes were not
introduced in this phase. Default and test-helpers runs failed only these targets:

- `config_flag_precedence::config_flag_overrides_env`, tests/config_flag_precedence.rs:59:
  expected completed-base display string absent. Code review shows canonicalization
  with dunce while fixture uses std::fs::canonicalize (Windows extended-path prefix
  differs); treat this as a path-comparison hypothesis, not proof of incorrect
  selection. A01 must verify effective destination and normalize the assertion.
  A separate temporary-fixture CLI probe using ordinary Windows paths exited 0,
  printed destination B selected by `--config`, did not print env destination A,
  and left source bytes unchanged. Its stdout lacked the extended-path prefix.
  This supports the display-form diagnosis; no test assertion was changed here.
- `move_dir_copy_full::move_dir_copy_fallback_preserves_all_files`, line 72:
  written bytes contain slash-separated relative paths; assertion expects Windows
  backslashes. Additionally its env hook does not force the library copy branch
  in integration builds because that hook is cfg(test). This failure does not
  demonstrate lost directory files. B04 is based on separate inspected code paths.

Formatting drift: src/app.rs, src/config/xml.rs, tests/cli_promote_file_to_dir.rs,
config_flag_precedence.rs, lock_eacces_unix.rs, locks_disabled_dir_unix.rs,
locks_disabled_file_unix.rs, move_dir_copy_full.rs, permission_denied_linux.rs,
root_cannot_move_base_unix.rs, root_placeholder_unix.rs, zfs_world_integration.rs.
No repository-wide formatting was applied.

Strict clippy: unused Component import in config/mod.rs; unused tracing::debug
in atomic.rs; unused code in dir_move.rs; unused Windows fsync_dir in util.rs;
two collapsible-if findings in config/paths.rs; collapsible-if and manual
is_multiple_of in lock.rs. Clippy stops before proving the rest of all-targets
warning-free. Tests also emit unused imports in copy_metadata.rs and
move_file_integration.rs. Do not add lint suppressions based on host-only usage.

## Lint flag evidence and repeatable override

.cargo/config.toml retains `[env] RUSTFLAGS = "-Cap-lints=warn"`. It was described
historically as a cap, but the measured default strict command still reports errors.
Do not assume that setting under `[env]` controls Cargo's own compilation flags.
The uncapped repeat saved both flag variables, set them to empty for just the
command, and restored them in finally:

```powershell
$savedFlags = $env:RUSTFLAGS
$savedEncoded = $env:CARGO_ENCODED_RUSTFLAGS
try {
    $env:RUSTFLAGS = ''
    $env:CARGO_ENCODED_RUSTFLAGS = ''
    cargo clippy --all-targets -- -D warnings
} finally {
    $env:RUSTFLAGS = $savedFlags
    $env:CARGO_ENCODED_RUSTFLAGS = $savedEncoded
}
```

This checkout has no configured target/linker flags to preserve in that override.
Other environments must inspect their flags first. A02 must establish effective
compiler arguments before changing local lint policy.

## Initial-phase verification and outstanding risks

GitHub automation removal covers all five tracked workflow files (including
semantic PR checks), release-please JSON, badges and active release documentation.
Cargo-dist metadata, local scripts, Docker, deny/audit policies and application
dependencies remain because they support local work. No lockfile or Rust source
change was needed. Repository issues/releases/discussions links are retained.
Removed obsolete workflow references from README backup without deleting its
potentially useful historical application documentation.

Reviewed final diff, documentation links, remaining automation references and git
status; preserved earlier user deletions. No commit/push/publish occurred.
`git diff --check` passed; all local links in the instruction/roadmap/audit files
resolve; no workflow YAML files or active README workflow links remain. A first
temporary CLI probe failed to launch using a relative executable path; rerunning
with the absolute executable path produced the observations above.

Not exercised: Linux/macOS native builds or runtime, aarch64, macOS universal
packaging, Unix all-features/xattrs, Docker/cross, dist/deb packaging, real-storage
ZFS tests, exhaustive fault injection or crash/power-loss testing. cargo-audit is
missing and cargo-deny policy is incompatible, so dependency security is unknown.
No dependencies were added/removed or policy relaxed. Source inspection findings
are documented risks, not reproduced destructive incidents. Prioritize B01–B05
before relying on automatic recovery, cross-device directory moves or resume.
