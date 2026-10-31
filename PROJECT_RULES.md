# aria_move project rules

Read with [AGENTS.md](AGENTS.md). Current design lives in
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md); maintenance task status lives in
[docs/REFACTORING_BACKLOG.md](docs/REFACTORING_BACKLOG.md).

## Purpose and compatibility

aria_move is a Rust 2024 binary and library on stable Rust (minimum 1.99). aria2 invokes it as
`aria_move <task_id> <num_files> <source_path>` to move completed downloads.
Preserving user data takes priority over speed and cleanup.

Preserve positional arguments, CLI flags, XML schema, public module paths and
re-exports (including prelude), and `AriaMoveError::code()` strings unless explicitly
authorized. Config selection is CLI flag > ARIA_MOVE_CONFIG > default path;
CLI values override XML settings.

## Architecture

| Location | Responsibility |
|---|---|
| src/main.rs | Parse arguments, call app, report failure and exit. |
| src/app.rs | Config merge, logging/signals, resolution and dispatch. |
| src/logging.rs | Binary-only tracing setup. |
| src/cli.rs, src/config/ | CLI and configuration types, XML, paths, validation. |
| src/fs_ops/ | Move/copy operations with a facade mod.rs. |
| src/platform/ | OS implementations with consistent exported signatures. |
| src/errors.rs | Typed errors and stable codes. |
| src/output.rs, src/shutdown.rs | Terminal output and cooperative cancellation. |
| src/utils.rs | Existing shared helpers; search before adding alternatives. |

Keep dependencies directed from main to app to operations. fs_ops must not depend
on app; platform code depends on crate-local errors rather than orchestration.
Keep config/mod.rs and platform/mod.rs as facades too.

## One implementation per responsibility

Before adding or replacing logic, search implementations, callers, tests, cfg
branches and re-exports. Compare behavior and failure semantics before choosing
what to reuse. Extend the established owner; do not introduce parallel loaders,
copy engines, naming rules, path checks or error adapters. If existing behavior is
wrong, fix it with regression coverage before consolidating callers.

When replacing an implementation, migrate all internal callers and remove the
obsolete private implementation, declarations, imports, tests and documentation.
Retain public compatibility adapters where required; do not leave two independent
policy implementations. Current ownership is documented in the architecture
reference; inspect current code before changing it.

Resolve configuration/environment once at the application boundary and pass
settings explicitly. Keep one XML mapping, CLI override policy, filesystem
publication policy and platform syscall adapter for each operation. Share specific
behavior, not unrelated functions merely called helpers. Do not introduce a
framework or wrapper layer to conceal competing implementations.

## Filesystem invariants

These are required safety contracts, not a claim that current code satisfies them.
Regression tests must exercise these contracts, including failure paths.

- Refuse replacement of all existing destinations. Try native exclusive rename first. On cross-device failure copy to a temp path in the
  destination directory, verify completeness and sync, publish by rename, then
  remove the source. Never remove source data before the destination is verified.
- Preserve collision handling and symlink/ancestor rejection through
  `path_has_symlink_ancestor`.
- Respect locks and config.disable_locks; resolve legacy lock environment only at
  the application boundary. Never silently proceed after a lock failure.
- Never infer artifact ownership from a filename or entry count. Leave unknown
  legacy artifacts untouched; clean up only uniquely owned staging.
- Dry runs create no directories, locks, temp files or other writes except the
  explicitly configured log file.
- Walks and chunked copies check shutdown::is_requested() and return
  AriaMoveError::Interrupted, preserving source data and recoverable partial state.
- Test relevant success, missing source, collisions, permissions, interruption
  and dry-run behavior using temporary directories.

## Errors, output and checks

Use AriaMoveError/AMResult conventions and existing io_error_with_help helpers.
New variants need code mappings and stable-code test coverage. Terminal output goes
through crate::output; diagnostics use structured tracing.

Run the general gate for Rust changes. On Linux/macOS also run clippy and tests
with --all-features. Do not use --all-features on Windows: xattr is Unix-only.
Release targets are Linux x86_64/aarch64, macOS universal and Windows MSVC.
Mark Unix/Linux-only integration files with appropriate crate-level cfg.
There is no local lint cap. Do not add one; strict clippy must remain blocking.
Measured results are in docs/VALIDATION.md.
Keep Cargo changes scoped, use existing dependencies, and run available local
license/advisory checks for dependency changes. Do not weaken security policy.

## Documentation ownership

AGENTS.md owns general Rust practice; this file owns project contracts; CLAUDE.md
imports those rules. docs/ARCHITECTURE.md records design decisions,
docs/REFACTORING_BACKLOG.md owns tasks, and docs/VALIDATION.md records dated
measurements. Use the backlog as the maintenance entry point. Update affected contracts,
examples and docs in the same change; avoid copied baselines in agent skills.
GitHub automation was intentionally removed. Preserve local build, test, lint,
security and packaging capabilities; do not reintroduce workflows without request.

## Change boundaries

Ask before unrequested dependency additions/removals or major upgrades, changes to
.github/workflows/, release config or deny.toml, and destructive work outside the
repo/temp directories. Existing explicit authorization applies. Never commit or
push unless asked. Use Conventional Commit titles.
