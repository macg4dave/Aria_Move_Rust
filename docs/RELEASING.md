# Local release packaging

Versions use three parts (for example 1.5.1) in Cargo, Debian and RPM metadata.
Targets: Linux amd64/arm64 (`.deb` and `.rpm`) and Windows x86_64.

## Docker release gate

From Linux or Git Bash with Docker Desktop running Linux containers:

```bash
bash scripts/docker-release-check.sh             # amd64 and arm64
bash scripts/docker-release-check.sh amd64       # one architecture
```

The command builds architecture-specific Debian 12 packages, runs formatting,
Clippy, unprivileged Rust tests and Lintian, then installs each exact package in
a disposable systemd Docker container. It checks clean installation, local HTTP
and real DNS/HTTPS downloads, completion hooks, crash/config recovery, restart,
upgrade with modified config, container restart/startup, existing aria2 integration
with the bundled service disabled, remove/reinstall and purge/reinstall.
It also runs the 2 GiB + ten 1 MiB concurrent-hook test on a separate completed
filesystem. All small moves must finish while the large copy is paused. Allow
at least 3 GiB free Docker RAM and about 2 GiB container disk for this check;
see [VALIDATION.md](VALIDATION.md) to run it separately.

Systemd containers require privileged execution and a Linux Docker engine with
writable private cgroups. The runner mounts test code read-only, copies the
package into the guest and never mounts the host Docker socket into the guest.
ARM64 on x86_64 uses Docker/QEMU emulation. Container restart is not a host
reboot; retain a native test-server reboot check before publishing when available.
If ARM64 containers fail with `exec format error`, enable the emulator in the
Docker engine once with `docker run --privileged --rm tonistiigi/binfmt --install arm64`,
then rerun the gate. This registers ARM64 execution in the Docker Linux VM.
The networked check defaults to `https://deb.debian.org/debian/README`; set
`ARIA_MOVE_TEST_URL` to another stable HTTPS text file if needed. Failures stop
the gate. Logs, package checksums and architecture PASS markers are written to
`target/release-check`. Publish these exact `.deb` files, without rebuilding them.

For every release, run this gate, the RPM gate below and `pwsh -File scripts/package-windows-release.ps1`
on Windows to test and archive the executable with README, docs and LICENSE. Create
`SHA256SUMS` covering every uploaded artifact. Commit the source, tag that commit
as `vVERSION` (for example `v1.5.1`), push it, then create the GitHub
release with the tested artifacts and an honest test/compatibility summary.
The gate does not upload, tag, or publish anything itself. No CI is required.

Build Debian packages on the target architecture using Rust 1.99+, a C linker,
debhelper and dpkg-dev:

```bash
bash scripts/build-linux-release.sh
```

Packages appear in the parent directory. scripts/docker-build.sh --deb uses
the same recipe in a Debian 12 Rust container, writing to target/debian.
Build ARM64 natively or with an ARM64 container and working emulation.
dh_shlibdeps calculates actual binary dependencies. Packages built on newer
distributions must not be advertised as supporting older glibc versions.

## RPM release gate

```bash
bash scripts/docker-rpm-check.sh                 # x86_64 and aarch64
bash scripts/docker-rpm-check.sh x86_64          # one architecture
```

Builds `aria-move-VERSION-1.el9.ARCH.rpm` from `packaging/rpm/aria-move.spec` on
AlmaLinux 9 (glibc 2.34 baseline), running the Rust tests in `%check`. Each RPM is
then installed in a disposable AlmaLinux 9 systemd container with EPEL aria2 and
checked by `tests/packaging/rpm-lifecycle.sh`: install (service left disabled, as
is RPM convention), enable, authenticated offline and HTTPS downloads through the
hook, crash restart, invalid-config recovery, upgrade with an edited conffile,
container restart, remove (data and token kept) and reinstall. Publish these
exact `.rpm` files from `target/release-check`.
Run `scripts/test-debian-package.sh` as described in [VALIDATION.md](VALIDATION.md)
before publishing each architecture.

Build Windows with cargo build --locked --release --target x86_64-pc-windows-msvc,
then archive the executable, README and LICENSE. Record SHA-256 hashes.
Publishing remains a separate manual step; no CI infrastructure is required.
