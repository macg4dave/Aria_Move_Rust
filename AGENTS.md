# AI agents: Rust development

Reusable guidance for Rust libraries, CLIs, and services. Read the manifest,
toolchain, available build configuration, and nearby code before implementing changes.
For this repository also follow [PROJECT_RULES.md](PROJECT_RULES.md).
Planned work belongs in the backlog linked from [ROADMAP.md](ROADMAP.md), not in permanent instructions.

## 1. Working approach

- Inspect git status first; preserve user edits and untracked work.
- Complete the requested scope and keep unrelated cleanup separate.
- Search existing implementations and callers before adding helpers.
- Prefer the smallest correct change. Explain assumptions affecting behavior.
- Preserve public APIs, CLI contracts, configuration formats and stored data unless
  the task explicitly authorizes a migration.

## 2. Validation

For Rust changes, run focused tests during development, then the default gate:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all --no-fail-fast
```

Follow the project's edition, MSRV, toolchain and formatting configuration.
Format changed files without unrelated formatting churn. Repository-wide formatting
is a separate change when there is existing drift.

Inspect lint flags: `--cap-lints=warn` can mask `-D warnings`. Use a documented,
command-local override when necessary, preserving required build/target flags.
Run the project's supported feature/platform matrix; `--all-features` may be invalid
for mutually exclusive or OS-specific features. For dependency changes run configured
license/security checks, such as `cargo deny check` and `cargo audit`, if available.
Report missing tools and untested targets.

Documentation-only changes normally need content, link and diff review rather than
compilation, unless project rules require more.

## 3. Definition of done

- Behavior changes have meaningful regression coverage and relevant failure tests.
- No new warnings or failing checks. Establish pre-existing failures with a measured
  baseline; historical notes alone are insufficient evidence.
- Review the final diff for accidental changes, stale references and secrets.
- Report files changed and why, commands actually run and their results, remaining
  failures, and untested platforms/features. Never claim an unrun check passed.

## 4. Rust implementation

- Prefer idiomatic ownership and borrowing: `&Path`, `&str`, and slices where
  ownership is unnecessary. Avoid gratuitous clones and lifetime complexity.
- Use typed errors where callers need matching; add operation/path context at
  application boundaries using existing error conventions.
- Avoid production panics for recoverable failures. An invariant-based `expect`
  needs an explanation. Document intentional best-effort error handling.
- Prefer safe Rust. Keep necessary unsafe blocks small and document actual safety
  invariants and unsafe API caller obligations.
- Pass settings explicitly rather than mutating process-global environment.
- Keep filesystem paths as `Path`/`PathBuf` or `OsStr`/`OsString`; never round-trip
  through lossy text. Display formatting is for messages.
- Prefer std and existing dependencies. Respect MSRV, licenses and feature policy;
  let Cargo update lockfiles rather than editing them by hand.
- Keep library output under caller control. Use existing structured diagnostics;
  never log secrets, sensitive contents or full environment dumps.

## 5. Architecture and lints

- Keep modules cohesive, visibility narrow and dependencies directional.
- Use meaningful domain names and named structs when tuples obscure intent.
- Treat file/function size as a review prompt, not a mechanical splitting quota.
- Preserve existing facade boundaries; avoid speculative abstraction.
- Document public contracts and module responsibilities. Explain decisions and
  invariants; remove obsolete comments and commented-out code.
- Fix warnings at their cause. Do not raise thresholds or hide dead code.
- Prefer narrow `#[expect(..., reason = "...")]` where MSRV supports it; avoid
  blanket suppressions and unrequested pedantic sweeps.

## 6. Tests

- Test observable behavior. Unit tests cover private logic; integration tests cover
  public boundaries. Prefer existing themed suites over tiny new test crates.
- Use temporary directories and deterministic fixtures. Ordinary tests must not
  touch real user data or make network calls.
- Prefer child-process environment overrides for CLI tests and injected settings
  for unit tests. Serial annotations alone do not prove environment mutation safe
  against other threads or dependencies. Unavoidable unsafe mutation needs a real
  process-wide safety argument and reliable restoration.
- Cover relevant success, failure, cancellation and no-side-effect behavior.
- Never weaken or ignore tests to get green; explain corrections to invalid tests.
- Keep setup readable; fallible tests can return Result and use `?`.

## 7. Platform and data safety

- Isolate substantial OS-specific behavior behind consistent interfaces. Check all
  supported targets before declaring host-unused code dead.
- Respect symlink, locking, overwrite, permission and durability policies.
- Establish replacement data is complete before removing originals. Account for
  failures between state transitions and define safe retry/recovery behavior.
- Keep dry runs free of unintended writes; check cancellation in long operations.
- Distinguish cross-compilation from runtime verification on another OS.

## 8. Refactoring playbook

Measure a baseline first. Identify callers, exports, cfg branches and symbol moves.
Refactoring preserves behavior; separate bug fixes and their regression tests.

| Operation | Procedure |
|---|---|
| Delete | Prove unused across targets/features and external API exposure; remove declarations/imports too. |
| Combine | Compare contracts and behavior first; preserve semantics and safety policy. |
| Split | Find responsibility boundaries; move verbatim first, narrow visibility, preserve public paths. |
| Join | Group by theme; preserve coverage/isolation and compare discovered tests/counts. Explain module-qualified name changes. |
| Move/rename | Update declarations, callers, docs and re-exports; preserve public paths or use an approved migration. |

Rerun the same checks and compare results. Report moved symbols as
`old::path -> new::path`, plus separate follow-up findings.

## 9. Baselines and follow-ups

Keep dated measurements and cleanup proposals in the roadmap or issue tracker.
Record revision, OS, toolchain, features, commands and evidence. An issue absent
from an old list is unclassified until investigated, not automatically a regression.
Roadmap entries are plans, not authorization to execute every item.

## 10. Scope and approval

Continue routine reversible work within the task. Ask before unrequested breaking
contracts, dependency additions or major upgrades, destructive actions, or release/
security policy changes. A requested multi-file refactor does not itself require
another approval. Explain the concrete decision when clarification is needed.

Do not commit, push, publish, bypass hooks or discard user changes unless asked.
Follow project commit conventions; use `refactor:` for behavior-preserving changes.
