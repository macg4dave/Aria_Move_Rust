# Local release packaging

Version 1.5 is represented as 1.5.0 in Cargo and Debian metadata.
Targets: Linux amd64/arm64 and Windows x86_64.

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

This replaces the hand-written DEB/RPM builder and unconfigured cargo-deb path.
The helper no longer produces RPMs; native RPM lifecycle support is not supplied.
Run `scripts/test-debian-package.sh` as described in [VALIDATION.md](VALIDATION.md)
before publishing each architecture.

Build Windows with cargo build --locked --release --target x86_64-pc-windows-msvc,
then archive the executable, README and LICENSE. Record SHA-256 hashes.
Publishing remains a separate manual step; no CI infrastructure is required.
