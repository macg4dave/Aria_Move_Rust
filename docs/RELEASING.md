# Release packages

Release binaries target Linux x86_64, Linux ARM64 and Windows x86_64. Linux
builds enable all features, including extended attributes. Windows uses MSVC.
Linux ZIP files contain `bin/aria_move`, `README.md` and `LICENSE`; the Windows
ZIP contains `bin/aria_move.exe` and the same documentation. A standalone Windows
executable is also provided.

Linux packages install `/usr/bin/aria_move`, documentation and the MIT license.
They do not create users, services or system configuration. The executable creates
a per-user configuration template on its first ordinary invocation.

Install the Debian package with `sudo apt install ./aria-move_<version>_<arch>.deb`.
Install the RPM with `sudo dnf install ./aria-move-<version>-1.<arch>.rpm`.
Debian architectures are `amd64` and `arm64`; ZIP/RPM architectures are `x86_64`
and `aarch64`. Linux artifacts are built on Debian 12 and require glibc 2.36 or
newer. They are not static musl binaries.

To reproduce Linux packages, mount a source copy at `/work` and an existing,
writable artifact directory at `/out` in `rust:1.99-bookworm`, then run
`bash /work/scripts/build-linux-release.sh`. Select Docker platform
`linux/amd64` or `linux/arm64` to match the desired architecture. Keep container
build directories separate from the host Cargo target directory. On x86_64,
ARM64 builds need QEMU/binfmt support (included in Docker Desktop).

The script builds with the locked dependency graph, creates ZIP/DEB/RPM files,
checks ZIP integrity, installs and removes both package formats in the disposable
container, and verifies executable startup. RPM installation uses `--nodeps` in
Debian because its installed libraries are tracked by dpkg; also verify the RPM
with DNF in a matching Fedora container before publication.

Build Windows with
`cargo build --locked --release --target x86_64-pc-windows-msvc` on Windows with
MSVC Build Tools. Package that executable without modifying its bytes.

Before publishing, run the checks in [VALIDATION.md](VALIDATION.md), verify each
artifact's version and architecture, and record SHA-256 hashes in `SHA256SUMS`.
The release tag must point to the source commit used for the binaries. Preserve
existing tags; bump Cargo.toml and Cargo.lock for a new release. Upload all assets
to a draft GitHub release, verify the uploaded hashes, then publish it. These are
local commands and do not require GitHub Actions.
