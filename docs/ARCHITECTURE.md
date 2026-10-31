# Architecture review and consolidation decisions

Reviewed 2026-10-31 at fa610d173f7701b76ce20143575ed22170c2243c on Windows MSVC.
This describes inspected code and proposed ownership, not completed fixes. Permanent
development rules live in [PROJECT_RULES.md](../PROJECT_RULES.md); implementation
tasks live in [REFACTORING_BACKLOG.md](REFACTORING_BACKLOG.md).

## Current execution and boundaries

`main -> app::run -> config selection/template -> XML merge -> logging/signals ->
validate_and_normalize -> resume::reconcile -> source resolution/promotion ->
move_entry -> move_file | move_dir`.

The library exposes config, cli, errors, fs_ops, platform, output, shutdown and
utils, plus root and prelude re-exports. Public helpers are compatibility surfaces,
even where production orchestration does not call them. There is no GUI, service
framework, database or network transfer engine; aria2 supplies completed downloads.

| Area inspected | Current implementation and assessment |
|---|---|
| main/app | Small entry point; app duplicates Args overrides, mutates environment, repeats diagnostic dispatch and promotes a nested file to its top-level download directory. Keep orchestration, remove duplicated policy after tests. |
| cli | Clap owns parsing and legacy positional forms; path sanitization removes legitimate quotes, converts non-Unicode paths lossily, and can turn `C:/` into `C:`. Override helper and app disagree on debug precedence, permissions and disable_locks. |
| config | mod.rs writes a second template and returns default settings as Loaded without parsing. xml.rs has tuple/Config loaders with differing errors and mapping. paths.rs and mod.rs have different symlink checks. validate.rs is undeclared, so its deprecated validation implementation is not compiled. |
| file move | Source-parent and destination locks, size probe, rename, then space check and staged copy. PermissionDenied locks silently downgrade protection. Rename helper can overwrite existing destinations; metadata errors are mostly ignored. |
| directory move | Separate rename/copy strategy; target chosen before destination lock. Multiple walks discard errors; regular files copied directly into visible target without per-file sync or verification; symlinks are omitted; source recursively removed afterward. Failure cleanup deletes target, which is not proven exclusively owned. |
| copy/io_copy | Retain staged copy and platform fast paths as starting points. Resume accepts any same-size temp or prefix at a destination-derived hash; it does not authenticate source identity/content. Streaming and kernel loops lack cancellation. Data mode does not explicitly sync data. |
| atomic/claim | atomic.rs pre-detects devices and syncs Unix parents, but Windows removes existing destination. claim.rs is compiled and unit-tested but has no production caller; its collision test does not force a collision. Do not introduce a second claim lifecycle. |
| resume | Binary-only startup cleanup deletes all matching temp files and destination trees judged partial by immediate entry counts. No ownership, age, lock or dry-run check. This conflicts with resumable copy and can erase unrelated or active work. |
| platform | Unix and macOS repeat log/mode/space adapters; common_unix/temp already share atomic config writes. Windows duplicates temp-name logic. Config functions named secure_new can replace existing config; Unix temp lacks final no-replace publication. Native APIs need explicit safety arguments. |
| space | platform check_disk_space and fs_ops/space repeat OS queries, while file and directory policies differ. Undeclared disk.rs is another implementation. Query errors become misleading InsufficientSpace with zero available. |
| utils/duplicate/helpers | utils holds timestamp naming, base protection and size probes. duplicate.rs exposes numeric naming with length handling, but movers use timestamp naming. helpers.rs formats errors and loses underlying error identity; it is not another generic utility module. |
| metadata | Shared implementation exists, but returns success on failed permissions/times/xattrs and documents a nonexistent strict parameter. Directory metadata is not restored in fallback; atomic file path tries reading xattrs from the vanished source. |
| logging/output/errors | Four subscriber construction branches repeat formatting; fallback and direct XML prints bypass output policy. JSON tracing coexists with human terminal messages. Error adapters flatten causes; from_io classifies every other error as permission denied. Preserve stable codes while correcting classification. |
| shutdown | Global flag and separate reason atomics; inconsistent Interrupted versus strings, blocking locks cannot stop, tests reset shared state without complete isolation. SIGTERM support is claimed but ctrlc dependency has no termination feature configured. Verify native behavior before documenting support. |
| tests | 66 integration source files plus unit/doc tests. CLI config fixtures repeat heavily; resume fixture covers prefix append but not identity; race tests can skip strict assertions, force-copy env is cfg(test)-only, global environment/shutdown tests have gaps in isolation. ZFS suite is opt-in and touches configured real storage; never run by default. |
| tooling/docs | Rust 2024 stable, no declared rust-version; Docker pins Rust 1.76. Lockfile ignored. Local release script ignores fmt/clippy and enables Unix feature on Windows. README backup duplicates stale guidance; run.stderr and tracked .DS_Store are hygiene candidates, not application code. |

## Target architecture

Keep the existing crate and facade paths. Use one resolved Config passed explicitly
from app into operations. Config owns selection (flag > env > default), one XML
mapping and one secure template writer. Args owns one override policy. Pure path
validation precedes all mutation; preparation is explicit and skipped for dry runs.
Source resolution owns bare-name fallback and promotion, with promotion semantics
characterized before moving them out of app.

fs_ops owns the move lifecycle: validate source, acquire ordered locks, reserve a
destination, attempt no-replace rename, stage on the destination filesystem on
EXDEV, verify and sync, publish without overwriting, then delete only the verified
source. A directory is staged as a whole tree, including empty directories; reject
unsupported entries explicitly. Do not infer success from a filtered walk. Recovery
must only act on provably owned artifacts under the same locking protocol. Start
by disabling heuristic deletion, then add minimal ownership metadata only if
resuming across processes requires it. No general transaction framework is needed.

| Responsibility | Retain/improve | Merge/replace/delete decision |
|---|---|---|
| Byte transfer | io_copy | Route directory files through the verified staged copy primitive; remove direct fs::copy alternative. Preserve bounded parallelism only after correctness. |
| Rename/durability | atomic + platform | One no-replace publication policy for file and directory moves; remove Windows pre-delete and duplicate directory rename handling. |
| Collision names | duplicate | Share name construction/length budgeting; preserve existing public numeric behavior and mover timestamp behavior initially. Choosing one visible suffix format needs compatibility approval. |
| Locks | lock | Centralize acquisition and explicit disable policy, resolve environment once; remove PermissionDenied fail-open branches. Maintain source-before-destination ordering and avoid locking same object twice. |
| Space queries | platform | Make space.rs policy/formatting only; delete disk.rs once absence from all module/build declarations is rechecked. Preserve query failure causes. |
| Symlink policy | config::paths + operation boundaries | One raw-path check with clearly defined leaf/ancestor semantics, used before writes; platform primitives must close race windows where possible. Keep public forwarding functions. |
| XML/templates | config::xml + platform writer | One fallible parser/mapping and template body; route mod.rs through it, retain legacy public signatures as adapters pending approved API migration. |
| Metadata | metadata | One documented best-effort policy initially, with warnings for failures; stricter semantics require explicit decision and failure tests. Preserve directory metadata after children. |
| Utility placement | fs_ops modules | Move naming to duplicate and stability probes to a cohesive fs_ops location only when callers justify it. helpers keeps error context. Do not merge unrelated helpers by filename. |
| Errors | errors + existing anyhow boundaries | Preserve typed causes/codes; contextualize rather than stringify. Keep public aliases for compatibility, use AriaMoveError/AMResult internally. |
| Dormant code | no new implementation | disk.rs and config/validate.rs are undeclared; claim_source and Windows delete_file_best_effort lack production callers. Remove only after cross-target/reference verification; do not remove public duplicate or lock APIs. |

## Verification boundaries

Static findings above describe concrete control flow, not reproduced data-loss
incidents. Native Unix/macOS, xattrs, cross-device mounts, power-loss durability,
permissions and signal delivery remain unverified here. Each backlog task requires
its own before/after regression evidence. Build success does not establish safe
filesystem transitions. No Rust application code was changed in this phase.
