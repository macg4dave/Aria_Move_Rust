# Refactoring backlog

Authoritative task plan, prepared 2026-10-31. See [architecture](ARCHITECTURE.md)
for evidence and ownership decisions, [validation](VALIDATION.md) for measured
results. Priorities: P0 data protection/check trust, P1 correctness/consolidation,
P2 maintenance. Risk describes implementation risk. Tasks are plans, not blanket
authorization; preserve public APIs and split behavior fixes from structural moves.

## Workstream A: establish reliable evidence

### A01 — Correct test assumptions (P0, medium risk; no dependencies)

Files: tests/config_flag_precedence.rs, move_dir_copy_full.rs, move_dir_basic.rs,
metadata_xattrs.rs, src/fs_ops/dir_move.rs. The Windows precedence assertion compares
display forms rather than effective destination; establish canonical equivalence
and destination selection before blaming config precedence. Directory fixture writes
slash strings but compares native paths. ARIA_MOVE_FORCE_DIR_COPY is only compiled
for library unit tests, so integration tests normally exercise rename instead.
Replace the env hook with private injected strategy tests or a controlled child
test harness; do not add a production force-copy option. Acceptance: tests prove
which path executed, compare original bytes, verify flag wins and env destination
stays untouched; run native Windows and Unix. Keep actual fallback fixes separate.

### A02 — Formatting and strict lint baseline (P0, low risk; no dependencies)

Files: .cargo/config.toml and files in VALIDATION.md. Separate formatting-only
change from lint fixes. Establish effective flags with verbose Cargo; config stores
RUSTFLAGS as an env setting and default strict clippy currently still fails. Remove
the cap only as an explicit local-check policy change, without dropping target flags.
Fix eight measured diagnostics at cause, then check any newly exposed binary/test
lints. Acceptance: fmt and uncapped clippy pass default/test-helpers on Windows and
default/all-features on Linux/macOS. No suppressions or threshold increases.

### A03 — Deterministic and isolated tests (P1, medium risk; A01)

Files: src/shutdown.rs, src/utils.rs tests, tests/config_env*, config_xml*,
locks_disabled*, fs_ops_race.rs, safe_copy_race.rs, claim tests, CLI suites.
Global shutdown resets race with nonserialized tests; environment restoration can
be skipped on panic; background writer thread is not joined; race tests can skip
their substantive assertions. Replace races with deterministic synchronization and
child-process env settings; share XML fixture setup in tests/common/mod.rs before
grouping suites. Acceptance: compare discovered test names/counts before/after,
repeat parallel execution, force collision/error paths and join workers. Do not
replace meaningful assertions with platform skips. ZFS remains opt-in.

## Workstream B: prevent data loss before structural consolidation

### B01 — Stop unowned recovery deletion (P0, high risk; no dependencies)

Files: src/resume.rs, app.rs, fs_ops/util.rs, copy.rs. Startup deletes matching
temps without checking active owners and removes existing destination directories
based on shallow entry counts. Remove heuristic deletion first; retain/report
unknown artifacts. Define owned staging identity and lock protocol before any
automatic recovery. Acceptance: unrelated same-name tree, active copy temp, equal
counts/different contents, inaccessible tree and interrupted prior operation all
preserve data. Retry must not erase previously published output. Use temp fixtures.

### B02 — Enforce dry-run across the full call chain (P0, medium risk; B01)

Files: app.rs, config/mod.rs, fs_ops/file_move.rs, dir_move.rs, lock.rs, resume.rs.
Config/template/validation and recovery write before move dry-run checks; source
lock acquired before dry-run creates Windows sidecar. Split pure validation and
preparation, gate recovery/template/directory/lock creation. Acceptance: snapshot
entire temp workspace for CLI and direct file/dir APIs with missing/existing config,
missing bases and stale artifacts; no writes except explicitly configured log.
Do not require writable destination merely to describe a hypothetical operation.

### B03 — No-replace publication and collision ownership (P0, high risk; A01)

Files: fs_ops/atomic.rs, file_move.rs, dir_move.rs, duplicate.rs, utils.rs,
platform/*.rs. Windows atomic helper removes destination; Unix rename replaces
existing entries; directory target chosen before lock; unique-name final fallbacks
are not checked. Implement exclusive reservation/no-replace publication, recalculate
names under lock, and never clean a target that this operation does not own.
Consolidate the atomic move path after safety tests. Acceptance: concurrent external
creator, mover contention, dangling symlink, exhausted suffixes, rename failure and
disabled locks never overwrite another entry. Existing public overwrite-policy
helper remains a naming API; changing its contract needs separate approval.

### B04 — Verified directory fallback (P0, high risk; A01, B03)

Files: fs_ops/dir_move.rs, copy.rs, io_copy.rs, metadata.rs, space.rs. Walk errors
and nonregular entries are discarded; direct fs::copy has no durability/length
check; source removal is unconditional after filtered copies. Stage entire owned
tree, propagate every enumeration failure, reject unsupported entries, verify
source identity/completeness, sync files and directories, publish then remove.
Replace direct copy implementation with shared verified copy primitive. Acceptance:
forced fallback with nested/hidden/empty trees, traversal denial, symlink/FIFO,
source change, short write, disk-full, publication failure and interrupted cleanup
preserves source and unrelated destinations. Partial source deletion after publication
must be reported with safe retry behavior; no recursive rollback of published data.

### B05 — Authenticate resumable files (P0, high risk; B01, B03)

Files: fs_ops/copy.rs, io_copy.rs, util.rs, tests/file_resume.rs. Temp name hashes
lossy destination only; existing same-size temp is published without checking
contents; shorter temp is trusted as a prefix. Match source identity and validated
prefix or restart in owned new staging; reject symlinks and stale artifacts.
Remove unchecked equal-size shortcut. Acceptance: same-length wrong data, changed
source, wrong prefix, temp symlink, oversized temp, concurrent owner and crash/retry
cannot publish corrupt content or delete source. Preserve public resume_temp_path
contract through compatibility adapter if naming changes.

### B06 — Cancellation and lock policy (P1, high risk; B03, B04, B05)

Files: shutdown.rs, fs_ops/lock.rs, io_copy.rs, file_move.rs, dir_move.rs,
utils.rs, app.rs. Blocking flock/Windows retry and io::copy/kernel loops cannot
stop; directory/probe return strings; lock denial silently proceeds. Use bounded
copy chunks and cancellable lock polling, resolve lock override once, propagate
Interrupted consistently. Avoid duplicate same-directory lock acquisition. Acceptance:
interrupt waiting/copying/walking/resuming and before publish/delete preserves
source and leaves only owned recoverable artifacts; permission denied fails unless
disable explicitly requested. Verify native SIGINT/SIGTERM behavior and documented
support; handler registration failure returns error instead of panic.

## Workstream C: one configuration and filesystem policy

### C01 — Unify config selection/loading/overrides (P1, medium risk; A01)

Files: app.rs, cli.rs, config/mod.rs, xml.rs, types.rs, tests/config_*, cli_*.
load_or_init returns defaults for existing files; app rereads XML via Option tuple,
unknown fields panic and other parse errors silently select defaults; boolean-only
XML may be ignored. Args overrides differ from app (disable_locks not applied in
app, debug precedence differs). Use one fallible path-based parse/map and one
override method; select flag/env/default explicitly without set_var. Replace tuple
internally, keep public loader adapters pending API decision. Acceptance: all fields,
boolean-only, empty/malformed/unknown XML, missing file, directory env override,
non-Unicode path and debug/log-level combinations; selection reads once and errors
cannot silently start moving with fallback settings.

### C02 — Consolidate secure config/path operations (P1, high risk; B02, C01)

Files: config/mod.rs, paths.rs, xml.rs, platform/common_unix.rs, temp.rs,
unix.rs, macos.rs, windows.rs, logging.rs, fs_ops/entry.rs. Duplicate templates
have different defaults; mod.rs uses File::create; secure_new adapters replace
existing files; symlink checks differ and may happen after mkdir; log leaf is not
protected by ancestor checks. Use one template, one pre-write leaf/ancestor policy
and native exclusive publication; share Unix adapters and temp names. Acceptance:
existing/readonly config unchanged, racing creator, leaf/ancestor symlink, dangling
link, injected write/sync failure and Windows reparse behavior. Escape XML template
paths. Delete obsolete template/check implementations only after callers migrate.

### C03 — One disk query and error model (P1, low risk; B04)

Files: fs_ops/space.rs, disk.rs, file_move.rs, platform/*.rs. Keep platform query,
make space policy/cushion shared by file/tree; remove duplicate syscalls and
undeclared disk.rs after checking declarations across targets. Query failure is not
zero free space. Acceptance: exact boundary/cushion/overflow tests, missing ancestor,
query failure with cause and native smoke tests on each OS.

### C04 — Source paths and naming (P1, medium risk; B03, C01)

Files: cli.rs, app.rs, fs_ops/resolve.rs, entry.rs, duplicate.rs, utils.rs.
Sanitizer mutates valid quoted/non-Unicode names and drive roots; resolver accepts
symlinks dispatcher rejects; promotion exists only on one app branch, so bare-file
fallback differs. Characterize intended aria2 forms and explicit-file promotion,
then centralize resolution and raw OsStr naming. Reuse duplicate length logic with
existing timestamp suffix policy; public numeric names unchanged. Acceptance:
single/three args, bare file/dir, nested file, external source, quotes, non-Unicode,
drive root, long extensions, collision exhaustion and symlink cases. A policy
change to promotion or visible suffix format requires explicit decision.

### C05 — Metadata semantics (P1, medium risk; B04, B05)

Files: fs_ops/metadata.rs, copy.rs, file_move.rs, dir_move.rs, tests/metadata*,
permissions_preserve.rs. Remove nonexistent strict-parameter documentation; make
best-effort behavior explicit with warnings, or propose strict behavior separately.
Avoid reading vanished source after rename; restore tree directory metadata after
children. Acceptance: permissions-only/full/off on rename and fallback, readonly,
timestamps, feature-gated xattrs and injected failure. Never claim preservation
based solely on Ok from an error-swallowing helper.

## Workstream D: simplify after correctness

### D01 — Preserve diagnostic causes and output consistency (P2, medium risk; C01, B06)

Files: errors.rs, fs_ops/helpers.rs, app.rs, logging.rs, output.rs, config/xml.rs.
String adapters erase io causes, from_io mislabels unrelated failures, repeated
dispatch and subscriber branches drift. Keep typed errors and anyhow context,
one diagnostic renderer and shared layer configuration; remove unconditional XML
debug prints. Clarify JSON versus human output contract before changing streams.
Acceptance: stable code tests, typed downcasts, nested causes, quiet/debug/JSON,
file-log failure and redirected stdout/stderr; no secrets in logs. Replace atty
with std::io::IsTerminal only with dependency approval and supported Rust version.

### D02 — Remove dormant alternatives (P2, low risk; C02, C03, A03)

Files: fs_ops/claim.rs, disk.rs, config/validate.rs, utils.rs, platform/windows.rs,
lib.rs. Recheck module declarations, cfg and public re-exports; disk/validate are
undeclared, claim_source and delete_file_best_effort have no production callers.
Delete redundant private paths and their obsolete imports/tests, rather than
activating them. Keep all exposed aliases/lock/naming APIs until approved migration.
Acceptance: symbol inventory records exact deletions, test count changes explained,
default/feature/native checks show no lost behavior; no blanket dead_code allowances.

### D03 — Reproducible local tooling and documentation (P1, medium risk; A02)

Files: Cargo.toml, .gitignore, Cargo.lock, docker/Dockerfile.build, scripts/*,
README.md, README.md.backup, deny.toml, .cargo/audit.toml. Docker Rust 1.76 cannot
parse edition 2024; local release script swallows fmt/clippy failure and uses
all-features on Windows; lock ignored, security config needs version validation.
Select declared MSRV/toolchain, align Docker and local gates, keep packaging local,
track Cargo-generated lock after approval; validate security tooling without
weakening policy. Remove confirmed junk separately. Acceptance: native build/test,
container build/test, local dist/deb smoke tests and license/advisory checks;
document unavailable tools/platforms. No GitHub automation is to be restored.

## Incremental delivery order

Start A01/A02 and B01, then B02/B03, B04/B05, B06. C01 may proceed independently
after A01; C02 follows its safety/config dependencies. Finish C03–C05, A03,
then D01/D02/D03 where dependencies permit. Each patch builds independently,
contains its meaningful regression tests, and reruns the same measured gate.
Preserve public forwarding paths during internal moves; do not combine behavior,
formatting, dependency policy and public API migrations in one patch.
