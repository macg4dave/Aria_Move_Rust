# Local validation

Use Rust 1.99 or newer with rustfmt, clippy and a native linker. No GitHub CI is
required. From the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets
cargo test --locked --workspace --all-targets --all-features
cargo test --locked --workspace --doc --all-features
```

On Windows, use the x86_64 MSVC toolchain and Visual Studio C++ Build Tools.
The optional xattrs feature adds Unix behavior only.

For Linux x86_64 and ARM64 runtime validation, use Docker:

```sh
bash scripts/test-linux.sh          # both architectures
bash scripts/test-linux.sh amd64    # x86_64 only
bash scripts/test-linux.sh arm64    # ARM64 only
```

Windows callers can run this through Git Bash with Docker Desktop. ARM64 uses
QEMU on x86_64 hosts; Docker must support `linux/arm64`. The script copies sources
into disposable containers, uses the image toolchain and runs as an unprivileged
user so permission tests execute. It tests default and all-feature configurations,
including documentation examples. Set `ARIA_MOVE_TEST_IMAGE` to override the Rust
image. Container images and Cargo dependencies may require network access during
setup; the ordinary tests themselves use isolated temporary files, not networks.

Unit tests cover configuration, resolution, collision naming, copying,
verification, metadata, locks and cancellation. Mover tests force both rename and
copy strategies without requiring a second filesystem. Integration tests exercise
the binary, concurrent processes, Unix signals/permissions, Windows junctions and
sharing rules. A real cross-device test probes for another filesystem (normally
`/dev/shm` in Linux containers); it reports a skip if none is available. A passing
Windows suite alone therefore does not prove real cross-volume runtime coverage.

The real-storage ZFS test is ignored by default and requires explicit opt-in:

```sh
ARIA_MOVE_RUN_ZFS_TEST=1 ARIA_MOVE_ZFS_ROOT=/your/test/mount \
  cargo test --test zfs_world_integration -- --ignored --nocapture
```

It creates only a uniquely owned temporary subtree. Without the configured mount,
it reports a skip. Ordinary validation does not require ZFS or administrator access.
macOS retains its native implementation; this pass adds no macOS test requirements.

Cross-compilation checks are optional supplements and do not substitute for runtime
tests. Use `cargo check --locked --all-targets --all-features --target <triple>`
after installing the relevant Rust target.
