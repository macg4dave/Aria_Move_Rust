# aria_move architecture

Rust 2024 binary and library, requiring Rust 1.99 or newer. The CLI preserves the
aria2 positional interface and delegates filesystem work to the public library.

## Execution and ownership

`main -> app -> configuration -> logging/signals -> validation -> source resolution
-> move_entry -> file or directory lifecycle`.

| Owner | Responsibility |
|---|---|
| app | Select flag/env/default config once, apply Args overrides, resolve legacy lock env once, install signals, dispatch and report. |
| config::xml | One fallible XML mapping, named LoadedSettings, one exclusive template writer; legacy optional tuple API is a compatibility adapter. |
| config::validate | One validation/preparation policy, re-exported through the config facade. |
| config::paths | Raw path/link checks, prospective normalization and creation of missing directories without changing existing modes. |
| fs_ops::resolve | Exact/bare source resolution before legacy quote decoding; CLI download resolution promotes nested files to their top-level download folder. |
| fs_ops::atomic + platform::filesystem | Native exclusive rename and publication; cross-device errors alone select fallback. |
| fs_ops::copy + io_copy | Exclusive owned staging, bounded cancellable transfer, identity/size/time checks, byte verification and file sync. |
| fs_ops::file_move / dir_move | One file/tree lifecycle, locks, shared copy primitive, publication and verified source cleanup. |
| fs_ops::duplicate | Raw-name budgeting and collision detection; existing public numeric and mover timestamp suffix formats remain distinct compatibility policies. |
| fs_ops::lock | Deterministically ordered, deduplicated cancellable locks; failures propagate unless explicitly disabled. |
| fs_ops::space + platform | One space policy over one syscall adapter per OS. |
| fs_ops::metadata | Shared best-effort metadata handling with diagnostics; tree directories are restored after children. |
| errors / output / logging | Stable typed classifications with retained I/O causes; stream-aware color policy and shared human/JSON tracing formatters. |
| shutdown | One atomic request/reason state; typed cancellation checkpoints. |

## Filesystem lifecycle

Same-filesystem moves use no-replace native rename: renameat2 on Linux,
renamex_np on macOS, MoveFileExW without replacement on Windows. A destination
created by another actor causes an error; it is never deleted to make room.

Cross-device moves create a unique staging artifact in the destination filesystem.
Files are copied and compared byte for byte. Tree enumeration propagates every
error, preserves empty directories and rejects links/special entries. Identity
and content are checked again before deletion. Files and directories are synced
before exclusive publication; source files are removed individually and source
directories only when empty. A failure after publication retains the published
copy. No rollback recursively deletes published or unowned paths.

Legacy destination-derived resume files and partial-looking folders are untrusted.
They are left untouched; transfers restart in fresh owned staging. The public
resume_temp_path helper remains for compatibility, not as proof of ownership.
There is no heuristic startup recovery/deletion pass.

Dry runs perform validation/resolution without creating config templates,
directories, locks or staging. Only an explicitly configured log may be written.
Config errors are returned before any move can start. Leaf links, Unix symlinks
and Windows reparse points are rejected at filesystem boundaries.

Metadata is best effort with warnings; atomic rename inherently carries existing
metadata. Full preservation takes precedence over permissions-only. Extended
attributes are a Unix feature. No public strict-metadata mode is implied.

## Compatibility and testing

Existing facade paths, CLI/XML shapes and error codes remain, with the approved
change that all move/copy publication refuses replacement. A new io_error code
classifies unrelated I/O failures without calling them permission errors.

Public CLI/API tests stay under tests/. Tests for private fallback selection,
copy verification, cancellation and source stability live beside their modules.
Shared fixtures remove repeated CLI XML setup. Environment tests use subprocess
settings; no test mutates the parent process environment or cwd. Cancellation
unit tests use one private panic-safe state guard. Real-storage ZFS testing is
ignored by default and requires explicit opt-in.

Private implementation moves include utils::unique_destination ->
fs_ops::duplicate::unique_destination, config::validate_and_normalize ->
config::validate::validate_and_normalize (public re-export retained), and duplicated Unix/macOS adapters ->
platform::unix/platform::filesystem. Public platform re-exports are unchanged.
Undeclared disk/validation alternatives, unused source claiming, heuristic recovery,
obsolete copy engines and bespoke temporary-name code were removed.
