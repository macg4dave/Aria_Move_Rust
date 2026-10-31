#!/bin/bash
# RPM counterpart of lifecycle.sh. Runs only inside a disposable systemd guest.
set -euo pipefail
test "$(id -u)" = 0
case $(cat /run/systemd/container) in systemd-nspawn|docker) ;; *) exit 2;; esac
package=/packages/package.rpm
upgrade=/packages/upgrade.rpm
# `! cmd` never fails under set -e, so negative checks use this instead.
refute() {
    if "$@"; then
        echo "Unexpected success: $*" >&2
        exit 1
    fi
}
boot_identity() {
    cat /proc/sys/kernel/random/boot_id
    awk '{print $22}' /proc/1/stat
}
download() {
    env -u ARIA_MOVE_TEST_URL python3 /package-tests/download.py
    if [[ -n ${ARIA_MOVE_TEST_URL:-} ]]; then python3 /package-tests/download.py; fi
}
active() {
    for ((i=0; i<100; i++)); do
        if systemctl is-active --quiet aria-move && test -s /run/aria-move/aria2.conf; then
            sleep 1
            systemctl is-active --quiet aria-move && return
        fi
        sleep .2
    done
    journalctl -u aria-move --no-pager
    return 1
}
case ${1:?expected before-reboot or after-reboot} in
before-reboot)
    refute rpm -q aria-move
    dnf install -y "$package"
    # RPM convention: installed, not enabled until the administrator says so.
    refute systemctl is-enabled --quiet aria-move
    test "$(stat -c '%a %U' /var/lib/aria-move/rpc-secret)" = '600 aria-move'
    test "$(stat -c '%a %U' /var/lib/aria-move/incoming)" = '750 aria-move'
    test "$(getent passwd aria-move | cut -d: -f7)" = /sbin/nologin
    runuser -u aria-move -- aria_move --check-config
    systemctl enable --now aria-move
    active
    test "$(stat -c %a /run/aria-move/aria2.conf)" = 600
    test "$(grep -c '^rpc-secret=' /run/aria-move/aria2.conf)" = 1
    refute grep -q rpc-secret= /proc/"$(systemctl show -p MainPID --value aria-move)"/cmdline
    test "$(systemctl show -p User --value aria-move)" = aria-move
    download
    # The packaged normal log level records completed moves.
    grep -q 'Move completed' /var/log/aria-move/aria_move.log
    systemctl kill --kill-whom=main --signal=KILL aria-move
    sleep 6
    active
    test "$(systemctl show -p NRestarts --value aria-move)" -ge 1
    # Invalid user configuration fails clearly; correcting it restores service.
    cp /etc/aria_move/config.xml /root/good-config.xml
    printf '<config><unknown/></config>\n' > /etc/aria_move/config.xml
    if systemctl restart aria-move; then
        echo 'Invalid configuration unexpectedly started' >&2
        exit 1
    fi
    cp /root/good-config.xml /etc/aria_move/config.xml
    systemctl reset-failed aria-move
    systemctl start aria-move
    active
    cp /var/lib/aria-move/rpc-secret /root/original-secret
    printf '\n# lifecycle local edit\n' >> /etc/aria_move/aria2.conf
    sha256sum /etc/aria_move/* > /root/config.sha256
    echo retained > /var/lib/aria-move/completed/retained.txt
    dnf install -y "$upgrade"
    active
    sha256sum -c /root/config.sha256
    test ! -e /etc/aria_move/aria2.conf.rpmnew
    cmp /root/original-secret /var/lib/aria-move/rpc-secret
    # Only the locally edited conffile may differ from the package.
    test -z "$(rpm -V aria-move | grep -v '/etc/aria_move/aria2.conf$' || true)"
    boot_identity > /root/old-boot
    ;;
after-reboot)
    test "$(cat /root/old-boot)" != "$(boot_identity)"
    systemctl is-enabled --quiet aria-move
    active
    download
    dnf remove -y aria-move
    refute systemctl is-active --quiet aria-move
    test ! -e /usr/bin/aria_move
    test -f /var/lib/aria-move/completed/retained.txt
    cmp /root/original-secret /var/lib/aria-move/rpc-secret
    dnf install -y "$package"
    systemctl enable --now aria-move
    active
    cmp /root/original-secret /var/lib/aria-move/rpc-secret
    download
    dnf remove -y aria-move
    echo 'PASS: install, enable, hook, restart, upgrade, reboot, remove and reinstall'
    ;;
*) exit 2;;
esac
