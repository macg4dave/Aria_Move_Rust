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

## Move logic regression coverage

The November 2026 concurrency tests can be run with:

```sh
cargo test --locked --test concurrent_hooks -- --nocapture
cargo test --locked concurrent_distinct_downloads_finish_for_both_strategies
```

The hook tests launch 24 separate binary processes using aria2's GID/count/path
arguments, sharing a configuration and verbose log. They mix distinct names,
Unicode, spaces, empty files, sizes around the 1 MiB copy buffer boundary and
multi-file download trees. They check exit status, every output byte, source
removal, staging/lock cleanup and completion records, with a 60-second deadline
and child cleanup on failure. Another test holds one source lock, verifies eight
sibling hooks finish while that hook waits, then releases it and checks that the
waiting hook finishes. The cross-filesystem batch reports a skip without a
second filesystem; the unit test always exercises concurrent forced copies as
well as renames. These tests invoke the hook binary directly; they do not launch
aria2 itself or simulate a network share.

To test explicitly authorized real storage, set `ARIA_MOVE_TEST_SHARE` to the
share's mount path (`W:\` in PowerShell, for example) and run:

```sh
cargo test --locked --test concurrent_hooks concurrent_hooks_on_real_share -- --ignored --nocapture
cargo test --locked --lib real_share_metadata_and_concurrent_forced_copies -- --ignored --nocapture
```

These tests create and remove only exclusively owned `aria_move_test_` temporary
subtrees. The hook test runs 72 processes: local-to-share, share-to-local and
share-to-share. The library test checks metadata across 120 reopens over three
seconds, then forces four concurrent verified copies of approximately 16 MiB
each within the share. Explicit cleanup reports failures.

Verified on 14 November 2026 against World at `W:` using the Windows SMB client:
both tests passed, including every byte comparison, source removal and subtree
cleanup. This does not establish Linux CIFS behavior: Docker Desktop could not
bind-mount the mapped UNC path, and Debian WSL awaited initial setup. Logs are
`target/world-share-hooks.log` and `target/world-share-copy.log`.

Source-change regression tests cover timestamp updates before and after copying,
including a same-length content update before copying. Timestamp-only snapshot
mismatches before publication discard staging and retry with a fresh inventory
and full verified copy, up to three attempts with a cancellable one-second pause.
Every refreshed inventory must retain the original sizes, identities and tree
entries. Tests verify recovery, exhaustion without publication, cancellation,
refusal of replacements/size/tree changes and source retention on byte differences
after publication. A timestamp-only change during cleanup requires a fresh
byte-for-byte comparison with the published destination, with stable snapshots
of both files around the comparison, before removing that source file.
Source snapshot errors include the old/new byte length, modification time and
filesystem identity so SMB/ZFS reports can distinguish the failing comparison.
These checks remain strict within each attempt: matching lengths alone do not
justify deleting a source. Post-publication failures never restart the move.

Verified on 14 November 2026 in an amd64 Linux Docker container as an
unprivileged user: default and all-feature Clippy, tests and documentation tests
passed. All three hook concurrency tests executed, including the real
cross-filesystem batch using `/dev/shm`; that check was skipped on Windows.

The October 2026 logic review added checks that single-file hooks and explicit
nested paths leave sibling downloads untouched, while multi-file hooks still
move their download folder. Both rename and copy paths accept completed .tmp,
.part and .crdownload files and refuse aria2 control files. Windows tests verify
that a pre-existing file at a lock path is neither changed nor deleted. Unix
tests distinguish unsupported directory sync from real storage/access errors.

Each hook streams files sequentially; independent hooks remain concurrent. The
Rayon pool and timed stability sampling were removed. Copy verification, source
change detection, exclusive publication and checked source deletion remain.
Actual SMB/NFS mounts are not exercised by the default Docker tests; use a
configured share before claiming compatibility with a particular server/mount.

Verified after these fixes on 31 October 2026: Windows all-feature tests and
Clippy passed; Docker amd64 and ARM64 all-feature tests, Clippy, Lintian and the
complete package lifecycle passed. The real-hook concurrency check also passed
on both architectures: ten small moves finished while the 2 GiB copy was paused,
and every SHA-256 matched. Logs: target/logic-review-windows-tests.log and
target/release-check/. These fixes are included in the refreshed v1.5.0 release.

## Debian package lifecycle

### Debian 13 virtual-disk move tests

```sh
bash scripts/test-disk-vm.sh
```

This builds `aria-move-disk-vm:debian13` and boots a real Debian 13 kernel under
QEMU TCG inside an unprivileged Docker container. KVM, Hyper-V access and host
disk mounts are not required. The guest has 1.5 GiB RAM, a disposable 4 GiB root
disk and three 1 GiB qcow2 data disks, formatted as ext4, XFS and Btrfs. It also
mounts a guest-local Samba export through the Linux CIFS client (SMB 3.1.1,
`cache=strict,actimeo=1`). It has no external network adapter.

The runner tests all 16 filesystem routes with eight concurrent hooks each,
including directory trees, Unicode names, empty files and copy-buffer boundary
sizes. It checks SHA-256 hashes, source removal and staging cleanup. Further
stages pause a 128 MiB copy while eight siblings finish, change only the
timestamp of a completed SMB file during copying and verify retry recovery,
and download eight files through aria2's actual completion hook. Every test
file is closed before the completion hook is invoked.

Build and guest console logs are saved under `target/disk-vm/`. The guest's
structured report and individual move logs are extracted there too. The
container and working disks are deleted after the run; the Docker image remains
for repeat runs. The guest timeout is 15 minutes. Building downloads Debian and
Rust packages and requires several GiB of free Docker storage. This test uses
Samba over ext4, not a ZFS server; World-share tests remain separate.

On 14 November 2026, this VM reproduced a timestamp-only change on a completed
Linux SMB file after publication: 4,095 bytes and the same identity, with an
approximately 17 ms mtime adjustment. Cleanup retained the source despite no
test writer modifying its bytes. The cleanup fix now reverifies bytes against
the published destination before accepting a timestamp-only refresh. Regression
tests retain sources on real content differences or replacement identities.
The subsequent full VM run passed all 146 hooks, including actual aria2 hooks,
all 16 filesystem routes, paused-copy independence and timestamp retry recovery.
The original failure is retained in `target/disk-vm-evidence/source-retained.log`.

The release gate also runs a heavy concurrency check using a real 2 GiB
download and ten 1 MiB downloads, invoked through aria2's actual completion
hooks. The completed directory is a separate 3 GiB tmpfs, forcing verified
copies. The large hook is paused after a partial staging copy is observed;
all ten small moves must finish before it resumes. Every output is SHA-256
verified and sources/staging must be removed. This requires at least 3 GiB
free RAM in Docker plus about 2 GiB of container disk space. To run only that
check against an already-built package:

```sh
bash scripts/docker-concurrency-check.sh amd64
bash scripts/docker-concurrency-check.sh arm64
```

Verified on 31 October 2026 with real aria2 completion hooks on Docker amd64
and ARM64 under QEMU: all ten small files completed before the paused large
copy resumed; all eleven output hashes matched, with no sources or staging
left behind. Native Linux and Windows Rust tests also cover source-specific
lock independence, duplicate calls, cancellation and concurrent file/directory
name collisions. The refreshed v1.5.0 release includes these fixes. The original v1.5.0
assets published earlier on the same day used directory-wide locking; replace
those installations with the refreshed binaries and verify SHA256SUMS.

The release gate is `bash scripts/docker-release-check.sh` (Linux or Git Bash).
It builds and tests the exact amd64/arm64 `.deb` files and retains logs, checksums
and PASS markers in `target/release-check`. See [RELEASING.md](RELEASING.md) for
Docker requirements and the separate Windows packaging command. The nspawn
workflow below remains useful for an additional independent test environment.

Also test a real hostname-based HTTPS download on a networked test server with
the bundled service running. Localhost-only tests cannot detect DNS failures in
the service sandbox:

```sh
sudo env ARIA_MOVE_TEST_URL=https://deb.debian.org/debian/README \
  python3 tests/packaging/download.py
```

Build the package, then test it in an ephemeral systemd-nspawn guest. The host
needs systemd-container and debootstrap. Create a clean rootfs once:

```sh
sudo debootstrap --include=systemd-sysv,dbus,python3,aria2,adduser trixie /var/tmp/aria-move-test-root https://deb.debian.org/debian
sudo bash scripts/test-debian-package.sh ../aria-move_1.5.0_amd64.deb /var/tmp/aria-move-test-root
lintian ../aria-move_1.5.0_amd64.deb
```

The runner boots a disposable copy, checks fresh installation and service-user
permissions, downloads from a guest-local HTTP server over authenticated RPC,
checks the actual completion hook, restarts, upgrades to a synthetic newer
package version, checks modified conffiles and credentials, reboots the guest,
and checks removal, reinstall, purge and reinstall after purge. It retains the
boot log in a printed temporary directory. Package operations never run on the
host. Network access is needed when preparing the rootfs; the guest has a private
network namespace and all download tests use its localhost. Repeat using an
ARM64 host/rootfs and ARM64 package before release.

As a supplementary ARM64 runtime check on an x86_64 Debian host, install
`gcc-aarch64-linux-gnu` and `qemu-user`, enable its binfmt registration, and run:

```sh
rustup target add aarch64-unknown-linux-gnu
CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUNNER=qemu-aarch64 \
QEMU_LD_PREFIX=/usr/aarch64-linux-gnu \
  cargo test --locked --workspace --all-targets --all-features --target aarch64-unknown-linux-gnu
```

Run package checks on the oldest distribution you advertise. The local Debian
13 amd64 build is not evidence of Debian 12 binary compatibility. A guest reboot
tests systemd enablement and startup, but is not a physical machine reboot.

Cross-compilation checks are optional supplements and do not substitute for runtime
tests. Use `cargo check --locked --all-targets --all-features --target <triple>`
after installing the relevant Rust target.
