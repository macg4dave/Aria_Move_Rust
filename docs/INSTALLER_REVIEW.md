# Installer review and verification

Reviewed 31 October 2026. The workspace already contained a draft native Debian
recipe and README rewrite; this work completed and exercised that design.

## Findings and implementation

The previous committed DEB builder staged only the executable and documentation.
It did not install configuration, a systemd unit, a service account or lifecycle
hooks. Its test only ran `--version`, so it could not catch the missing working
setup. The README required users to supply wrapper scripts and system files.
The same builder also produced an RPM without equivalent lifecycle integration.
These are concrete design defects; without the originally failing package and
its installation log, they do not establish the exact error observed by a user.

The replacement uses debhelper 13, calculated shared-library dependencies, dpkg
conffiles and standard generated systemd maintainer scripts. See Debian's
[dh_installsystemd source and documentation](https://github.com/Debian/debhelper/blob/master/dh_installsystemd).
The build helper normalizes package metadata permissions on Windows mounts.
It no longer publishes a binary-only DEB or an incomplete RPM.

The service owns its state/log directories, runs without capabilities as
`aria-move`, restricts filesystem writes and home access, waits for networking,
restarts on failure and gives aria2 time to shut down and save its session.
The mover remains an executable completion hook, rather than a second daemon.
The internal launcher has one packaging purpose: generate a private persistent
RPC token atomically and compose a private runtime aria2 configuration without
exposing the token in argv or world-readable conffiles. Users create no scripts.
RPC follows the upstream [aria2 manual](https://github.com/aria2/aria2/blob/master/doc/manual-src/en/aria2c.rst).

First-run CLI setup supports exclusive config creation with XML escaping and
saved preservation/locking options. Empty config paths are rejected. Missing
configuration during a download hook returns failure with setup instructions,
instead of reporting success after creating a template without moving anything.
Configuration validation is explicitly read-only and does not claim to validate
write permissions. Linux installation paths stay in platform-specific packaging.

Removal stops/disables the unit; purge removes conffiles. Data, logs, credentials
and the restricted account remain deliberately, including after purge. Reinstall
reuses them. The README and installation guide describe this retention policy.

## Verification

- Windows x86_64: formatting, Clippy with all features, 74 unit tests, 16 CLI
  tests, 4 move tests and the documentation test passed.
- Linux x86_64: Clippy with all features, 84 unit tests, 20 CLI tests, 5 move
  tests and the documentation test passed. The opt-in real ZFS test was not run.
- Linux ARM64: cross-compilation and QEMU execution passed all 84 unit, 20 CLI
  and 5 move tests with all features. This is emulation, not native hardware.
- Debian 13 amd64: native DEB build and lintian passed without diagnostics.
- A disposable Debian 13 systemd-nspawn guest with a private network passed
  fresh apt installation, service-user permissions, authenticated localhost RPC
  download, actual completion-hook move, crash recovery, invalid-config failure
  and correction, restart, synthetic newer-version upgrade, modified conffile
  and credential preservation, guest reboot/startup, removal, reinstall, purge
  and reinstall after purge with retained data.

The test package is built against Debian 13 libraries. Debian 12 compatibility
requires building and testing in the documented Debian 12 build container.
Subsequent [smoke101 ARM64 tests](SMOKE101.md) covered native package building,
clean installation of the bundled service, DNS/TLS downloads, restart and a
server reboot. Those tests found and fixed a DNS socket-family restriction in
the bundled unit; see [major issues](../issues.md). Full ARM64 upgrade/removal
lifecycle coverage remains a separate release gate.
Windows Rust execution was tested; Windows aria2 startup/hook integration was
reviewed but not exercised against a Windows aria2 installation. No physical
host reboot, macOS testing or CI infrastructure was added.
