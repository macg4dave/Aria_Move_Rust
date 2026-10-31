#!/usr/bin/env bash
# Boot an ephemeral copy of a clean Debian rootfs; never install on the host.
set -euo pipefail
if [[ $# != 2 || $EUID != 0 ]]; then
    echo 'Usage: sudo bash scripts/test-debian-package.sh PACKAGE.deb DEBIAN_ROOTFS' >&2
    echo 'Rootfs must contain systemd-sysv, dbus, python3, aria2, adduser and apt.' >&2
    exit 2
fi
package=$(realpath "$1")
rootfs=$(realpath "$2")
test "$rootfs" != /
test -f "$rootfs/etc/debian_version"
test -x "$rootfs/sbin/init"
repo=$(cd "$(dirname "$0")/.." && pwd)
stage=$(mktemp -d /var/tmp/aria-move-lifecycle.XXXXXX)
machine="aria-move-test-$$"
install -m644 "$package" "$stage/package.deb"
cleanup() {
    machinectl terminate "$machine" 2>/dev/null || true
    wait "${guest_pid:-}" 2>/dev/null || true
    echo "Guest boot log: $stage/boot.log"
}
trap cleanup EXIT
systemd-nspawn --directory="$rootfs" --ephemeral --boot --private-network --machine="$machine" \
    --bind-ro="$stage:/packages" --bind-ro="$repo/tests/packaging:/package-tests" \
    > "$stage/boot.log" 2>&1 &
guest_pid=$!
ready() {
    for ((i=0; i<120; i++)); do
        if systemd-run --machine="$machine" --wait --pipe --quiet /usr/bin/systemctl is-system-running; then
            return
        fi
        kill -0 "$guest_pid" || { cat "$stage/boot.log"; return 1; }
        sleep 1
    done
    cat "$stage/boot.log"
    return 1
}
ready
systemd-run --machine="$machine" --wait --pipe /bin/bash /package-tests/lifecycle.sh before-reboot
machinectl reboot "$machine"
sleep 3
ready
systemd-run --machine="$machine" --wait --pipe /bin/bash /package-tests/lifecycle.sh after-reboot
