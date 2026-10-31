# Validation

Measured 2026-10-31 against base revision
`9c6d5e4e4b3b64460183d632c72915f7e9ab3809` and the completed working tree.
Rust/Cargo 1.99.1; Windows MSVC host and unprivileged Linux x86_64 container.

| Executed check | Result |
|---|---|
| cargo fmt --all -- --check | Pass |
| cargo clippy --all-targets -- -D warnings | Pass on Windows and Linux |
| Windows clippy with --features test-helpers | Pass |
| cargo test --all --no-fail-fast | Windows: 115 passed; Linux: 164 passed, 1 ignored |
| Windows tests with --features test-helpers | 115 passed |
| Linux clippy/tests with --all-features | Pass; 166 tests passed, 1 ignored |
| cargo check --all-targets --all-features --target x86_64-apple-darwin | Pass, cross-compilation check |
| Same check for aarch64-apple-darwin | Pass, cross-compilation check |
| Same check for aarch64-unknown-linux-gnu | Pass, cross-compilation check |
| cargo deny --config deny.toml check --hide-inclusion-graph | Pass: advisories, bans, licenses and sources |
| cargo audit | Pass; no vulnerabilities reported |
| Dockerfile.build image build | Pass with Rust 1.99 |
| Container build/check/test script with MAKE_DEB=true | Pass; Debian package produced |
| dist manifest/build --tag v1.0.0 --artifacts=local --target x86_64-pc-windows-msvc | Pass; ZIP and SHA-256 produced |
| Shell script syntax and git diff --check | Pass |

All test totals include doctests. The ignored Linux test is the explicitly gated
real-storage ZFS test; it was not executed. cargo-deny reports an informational
upstream syn 2/3 version duplication from Windows timezone support and current
proc macros. No advisory was ignored or security policy relaxed.

The fresh pre-change Windows baseline was fmt failure, eight strict clippy errors,
and 113 passing/two failing tests. The previously failing precedence and directory
fixture tests now pass. Private copy/claim/recovery/probe tests were replaced or
removed with their implementations; new deterministic transfer, publication,
cancellation and data-preservation coverage replaces them. Test discovery was
compared before and after; 65 integration source files remain. This was a behavior
correction and consolidation, not a test-count-preserving file-only move.

Regression checks exercise real cross-device transfers between /tmp and /dev/shm,
empty/hidden/nested trees, mode/mtime/xattr preservation, concurrent publication,
SIGTERM while waiting for locks, unreadable trees, literal quoted and non-Unicode
names, stale/corrupt legacy partials, malformed/boolean-only config, missing
config/destinations, dry-run workspace snapshots and structured JSON output.
Environment and cwd overrides run only in child processes. Private fallback tests
use injection instead of a production environment switch. No real user data or
network is used by ordinary application tests.

Native macOS runtime/universal packaging and native Linux ARM runtime were not
available; successful cross-checks do not establish runtime verification there.
No power-loss simulation or real ZFS test was performed. No GitHub workflows were
restored and nothing was committed, pushed or published. The validation/ directory
retains the original audit's historical baseline logs, not current failures.
