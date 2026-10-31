# smoke101.lan ARM64 smoke test

Test date: 31 October 2026. Target: Debian 13.7 ARM64 (`aarch64`).

Testing first covered the existing `aria2.service`, followed by a complete
cleanup and fresh bundled-service installation at the user's request. The
server now runs only the packaged `aria-move.service`. Earlier phases below
are historical. Passwordless sudo for `dave` remains enabled as requested.

## Routine fixes

- The Git archive exported `debian/source/format` with CRLF on Windows, causing
  `dpkg-source` to reject `3.0 (native)`. `.gitattributes` now applies the LF rule
  throughout `debian/**`, including nested metadata. Archive output was checked.
- Existing aria2 configuration referenced nonexistent `/mnt/World/incoming/`
  and a missing `/usr/local/bin/aria_move.sh`. Integration uses the existing
  writable `/var/lib/aria2` service storage and the installed executable hook.

No major defect requiring `issues.md` was found during initial setup.

## Installation and download verification

Built `aria-move_1.5.0_arm64.deb` natively on the server with Rust 1.99.1.
The build passed 84 unit tests, 20 CLI tests, 5 move tests and the documentation
test. The opt-in ZFS storage test was not run. Lintian emitted no diagnostics.
Installation upgraded an existing binary-only `aria-move` 1.0.1 package.

The bundled `aria-move.service` was masked before installation to prevent a
second aria2 process competing for port 6800. Debhelper printed an expected
preset warning for the masked unit; package installation completed successfully.
The existing `aria2.service` remains the service to manage.

Before changing configuration, checked that aria2 had no active or queued
downloads and backed up its configuration, unit and session to
`/root/aria-move-smoke-backup-20261031-135922`. Only the incoming directory and
completion-hook settings were changed in aria2.conf; existing RPC credentials
and other settings were retained.

- Incoming: `/var/lib/aria2/downloads`
- Completed: `/var/lib/aria2/completed`
- Hook: `/usr/bin/aria_move`
- Mover config: `/etc/aria_move/config.xml`
- Move log: `/var/lib/aria2/aria_move.log`

Submitted `https://deb.debian.org/debian/README` through the existing service's
authenticated localhost RPC. The service downloaded it over HTTPS and invoked
the mover, which placed the 1,198-byte file in the completed directory and
removed it from incoming. Compared its bytes with a separately fetched HTTPS
copy. SHA-256: `a0c66a1fe046f008b6db9584d05347dd8ded2b78250851b61d3861c213ac361e`.
The first verified file is
`/var/lib/aria2/completed/aria-move-smoke-b6389df56f1e.txt`.

An initial test URL, `https://www.debian.org/robots.txt`, returned HTTP 404;
switching to the repository README resolved this external test-input problem.

Repeated the same download successfully after `systemctl restart aria2`.
The service was confirmed enabled and running as `aria2`; `dpkg --audit` was
clean and `visudo -c` passed. Both downloaded test files were left in completed
storage for inspection. No server reboot or package removal was performed.

## Explicit .deb reinstall test

Rebuilt the ARM64 `.deb` with corrected post-install messages: they now describe
the bundled RPC endpoint conditionally and explain how existing aria2 services
are managed. Previously the output incorrectly implied the bundled RPC server
was running even when the bundled unit was masked.

Ran the normal package operation directly on smoke101.lan:

```sh
sudo apt-get install --reinstall -y /home/dave/aria-move-smoke/aria-move_1.5.0_arm64.deb
```

Kept the existing aria2 integration selected earlier. The initial reinstall
preserved configuration and passed another real HTTPS download/move test.
Replaced the temporary bundled-service mask with the normal disabled state and
repeated the reinstall. This removed the preset warning. The package correctly
left `aria-move.service` disabled/inactive and `aria2.service` enabled/active.

Checksums of the mover config, existing aria2 config and existing service unit
were unchanged after both reinstalls. A further HTTPS download passed after
the final reinstall: `/var/lib/aria2/completed/aria-move-smoke-ac92a11883d4.txt`,
1,198 bytes with the same verified SHA-256 listed above. `dpkg --audit` and
Lintian were clean. This server test covers package reinstallation with an
existing aria2 service; it does not claim a fresh bundled-service installation.

No major issue requiring `issues.md` was found. Passwordless sudo remains enabled
as requested, and existing downloads remain intact.

## Final clean installation of the bundled service

At the user's explicit request, purged both aria-move and aria2, removed the old
aria2 unit/configuration, all aria2/mover data and logs, old test downloads,
the earlier configuration backup, service overrides and the old service
accounts. Confirmed the mover executable/configuration were absent and port
6800 was free. Kept SSH, sudo access and build/test artifacts.

Installed the ARM64 `.deb` using APT from this clean baseline. APT installed
aria2 as a dependency, created the restricted service account, registered and
enabled `aria-move.service`, and started it automatically without manual setup.

The first clean-install HTTPS test exposed a major DNS sandbox defect:
`RestrictAddressFamilies` excluded `AF_NETLINK`. The resolver could not enumerate
interfaces, so ordinary hostname downloads failed with aria2 error 19 even
though DNS worked outside the service. A diagnostic override confirmed the
cause. Fixed the packaged unit to allow `AF_NETLINK`, removed the diagnostic
override, rebuilt the `.deb`, then repeated the full cleanup and fresh install.
See [issues.md](../issues.md) for the resolved major defect.

The corrected clean installation passed:

- Default config validation as the restricted `aria-move` account.
- Authenticated RPC, local HTTP download and automatic completion-hook move.
- Hostname-based HTTPS download with byte-for-byte verification against a
  separately fetched source, both before and after service restart.
- An actual server reboot: new boot ID confirmed, service automatically active
  and enabled, persistent RPC token unchanged, and another HTTPS download/move
  verified successfully.
- Private token and runtime config modes both `0600`, owned by `aria-move`.
- No unit overrides (`DropInPaths` empty), no old `aria2.service`, clean package
  audit, and no manual config edits needed after installing the fixed package.

The final post-reboot file is
`/var/lib/aria-move/completed/aria-move-smoke-e212058cdc22.txt` (1,198 bytes,
SHA-256 `a0c66a1fe046f008b6db9584d05347dd8ded2b78250851b61d3861c213ac361e`).
Manage the installation with `systemctl status aria-move` and inspect moves in
`/var/log/aria-move/aria_move.log`. Current configuration is in `/etc/aria_move`;
incoming/completed storage is under `/var/lib/aria-move`.

The package download test now also supports `ARIA_MOVE_TEST_URL` for real
DNS/TLS regression checks, while retaining its offline localhost mode. Both
modes were exercised against the corrected installation.

## Clean install: logging rework and install-time RPC secret

Built `aria-move_1.5.0_arm64.deb` natively from the working tree (Rust 1.99.1,
`dpkg-buildpackage -b -d` because Rust comes from rustup): 88 unit, 23 CLI and
6 move tests plus the doc test passed; Lintian was clean. Purged aria-move and
aria2, removed `/etc/aria_move`, `/var/lib/aria-move`, `/var/log/aria-move` and the
service account, confirmed port 6800 free, then installed with APT.

- `rpc-secret` created by postinst before first start; it and the runtime
  `/run/aria-move/aria2.conf` are `0600 aria-move`. The runtime config has exactly
  one `rpc-secret` line, the conffile none, and aria2's argv carries no token.
- Wrong token rejected (`Unauthorized`); offline and HTTPS
  (`ARIA_MOVE_TEST_URL=https://deb.debian.org/debian/README`) RPC downloads moved.
- Logging at the default `warn`: successful moves left the log file empty; a
  failed move was written to both `journalctl -t aria_move` and the log file.
