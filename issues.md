# Major issues

## Resolved: clean bundled-service installation could not resolve download hosts

Found on 31 October 2026 during a clean ARM64 `.deb` installation on smoke101.lan
(Debian 13.7). The service started and accepted RPC, but HTTPS downloads failed
with aria2 error 19, "Name resolution ... failed: No address returned". DNS
worked outside the service. This blocked ordinary downloads after installation.

The service socket-family allowlist excluded `AF_NETLINK`, which the resolver
needs to enumerate network interfaces. Adding it restored DNS and the same HTTPS
download completed and moved successfully. The packaged unit now permits
`AF_NETLINK` while retaining the restricted service account and other sandboxing.

The earlier localhost-only lifecycle download test could not detect this defect.
The test now supports an explicit `ARIA_MOVE_TEST_URL` mode for real DNS/TLS
downloads. A second clean installation of the rebuilt `.deb`, without diagnostic
overrides, passed HTTPS download/move checks, restart and an actual server
reboot with automatic service startup and another verified HTTPS download.
