# Second implementation pass — 31 October 2026

Reviewed the application flow, Rust modules, existing unit/integration tests,
platform primitives, configuration, dependencies and local build/test scripts.

## Implemented changes

- Source resolution rejects linked ancestors before canonicalization can hide
  them while promoting a nested file to its download folder.
- The shared inventory rejects an adjacent `<source>.aria2` control entry for
  files and directories, including dry runs. Inspection errors propagate.
- Copy cleanup reopens and checks each source entry immediately before removal.
  Changed files, replacement identities and unexpected directory children are
  retained. Shared entry opening keeps inventory and cleanup type/link policy
  consistent. Cleanup remains non-recursive.
- Destination naming reports inspection failures rather than interpreting every
  error as a free name. Dangling links remain occupied. The suffix search no
  longer ends in an `expect` panic.
- XML parsing validates a complete single `<config>` root, rejecting wrong roots,
  multiple roots and trailing text that Serde alone accepted.
- CLI source resolution borrows paths instead of cloning them. The lone path
  argument uses `OsString`, preserving Unix non-Unicode filenames.
- Logging uses a typed threshold instead of parsing filter directives. Removed
  the `env-filter` feature and four unused transitive crates: `matchers`,
  `regex-automata`, `regex-syntax` and `aho-corasick`.
- Linux validation uses the image toolchain rather than downloading a duplicate
  stable toolchain for the unprivileged test user, and now includes doctests.
- Fixed stale README config-error behavior, a systemd config filename, a broken
  code fence and Docker testing instructions. Added the previously missing
  architecture and local validation documents.

No modules were merged or removed: their current responsibilities remain useful.
Replaced naming/filter implementations were removed directly, with callers
updated and no compatibility wrappers.

## Regression coverage

Added tests for adjacent aria2 controls across both move strategies and dry-run
settings; source changes and identity replacement during cleanup; unexpected
directory children; invalid destination paths; dangling destination links; linked
source ancestors; and single-argument non-Unicode CLI paths. Extended existing
XML invalid-document cases. Serialized the stability test with other tests that
observe the process-wide cancellation flag, removing a parallel-test race.

## Validation

Default and all-feature configurations are checked separately. Documentation
examples are checked with all features. Linux containers run as UID 1000 so
permission tests execute rather than skipping for root.

| Platform | Runtime environment | Default tests | All-feature tests | Doctests |
|---|---|---:|---:|---:|
| Windows x86_64 | Native MSVC | 93 passed | 93 passed | 1 passed |
| Linux x86_64 | Docker | 107 passed | 108 passed | 1 passed |
| Linux ARM64 | Docker/QEMU on x86_64 | 107 passed | 108 passed | 1 passed |

All three platforms passed formatting and clippy with warnings denied, for
both feature configurations. Both Linux target triples also passed
`cargo check --locked --workspace --all-targets --all-features` from Windows.
The Windows cross-device regression was separately run with `--nocapture` and
used a real second filesystem; it did not skip. Linux cross-device tests use
the separate `/dev/shm` filesystem.

Local logs are under `target/`: `windows-default-final.log`,
`windows-final.log`, `linux-amd64-final.log` and `linux-final.log` (the latter
includes the ARM64 run). These generated logs are intentionally untracked.

The opt-in real-storage ZFS test was not executed; it remains ignored. No macOS
runtime tests were added or run. ARM64 emulation is runtime validation, not a
claim of testing on physical ARM hardware. Path inspection and inventory sampling
remain subject to concurrent-writer race windows, as documented in
[architecture](ARCHITECTURE.md).

Repeatable commands and prerequisites are in [validation](VALIDATION.md).
