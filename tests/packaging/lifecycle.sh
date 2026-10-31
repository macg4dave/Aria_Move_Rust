#!/bin/bash
# Runs only inside a disposable systemd guest. Never on the host.
set -euo pipefail
test "$(id -u)" = 0
case $(cat /run/systemd/container) in systemd-nspawn|docker) ;; *) exit 2;; esac
export DEBIAN_FRONTEND=noninteractive
package=/packages/package.deb
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
    ! dpkg-query -W -f='${Status}' aria-move 2>/dev/null | grep -q 'install ok installed'
    apt-get install -y "$package" python3
    active
    systemctl is-enabled --quiet aria-move
    runuser -u aria-move -- aria_move --check-config
    test "$(stat -c %U /var/lib/aria-move/incoming)" = aria-move
    test "$(stat -c %a /var/lib/aria-move/rpc-secret)" = 600
    test "$(stat -c %a /run/aria-move/aria2.conf)" = 600
    test "$(grep -c '^rpc-secret=' /run/aria-move/aria2.conf)" = 1
    test "$(systemctl show -p User --value aria-move)" = aria-move
    download
    systemctl kill --kill-whom=main --signal=KILL aria-move
    sleep 6
    active
    test "$(systemctl show -p NRestarts --value aria-move)" -ge 1
    systemctl restart aria-move
    active
    # Invalid user configuration fails clearly; correcting it restores service.
    cp /etc/aria_move/config.xml /root/good-config.xml
    printf '<config><unknown/></config>\n' > /etc/aria_move/config.xml
    if systemctl restart aria-move; then
        echo 'Invalid configuration unexpectedly started' >&2
        exit 1
    fi
    systemctl stop aria-move
    cp /root/good-config.xml /etc/aria_move/config.xml
    systemctl reset-failed aria-move
    systemctl start aria-move
    active
    cp /var/lib/aria-move/rpc-secret /root/original-secret
    printf '\n# lifecycle local edit\n' >> /etc/aria_move/aria2.conf
    sha256sum /etc/aria_move/* > /root/config.sha256
    echo retained > /var/lib/aria-move/completed/retained.txt
    # Build a strictly newer version to exercise upgrade maintainer scripts.
    dpkg-deb -R "$package" /root/upgrade
    version=$(dpkg-deb -f "$package" Version)
    sed -i "s/^Version: .*/Version: ${version}+lifecycle1/" /root/upgrade/DEBIAN/control
    dpkg-deb --root-owner-group -b /root/upgrade /root/upgrade.deb
    apt-get install -y /root/upgrade.deb
    active
    sha256sum -c /root/config.sha256
    cmp /root/original-secret /var/lib/aria-move/rpc-secret
    boot_identity > /root/old-boot
    ;;
after-reboot)
    test "$(cat /root/old-boot)" != "$(boot_identity)"
    active
    download
    # Existing aria2 integration: a disabled bundled service must stay disabled
    # when the package is reinstalled, without disturbing the independent unit.
    cp /run/aria-move/aria2.conf /var/lib/aria-move/existing.conf
    chown aria-move:aria-move /var/lib/aria-move/existing.conf
    systemctl disable --now aria-move
    cat > /etc/systemd/system/aria2-existing.service <<'UNIT'
[Unit]
Description=Existing aria2 installation used by package tests
[Service]
User=aria-move
Environment=ARIA_MOVE_CONFIG=/etc/aria_move/config.xml
ExecStart=/usr/bin/aria2c --conf-path=/var/lib/aria-move/existing.conf
[Install]
WantedBy=multi-user.target
UNIT
    systemctl daemon-reload
    systemctl enable --now aria2-existing
    apt-get install --reinstall -y /root/upgrade.deb
    ! systemctl is-enabled --quiet aria-move
    ! systemctl is-active --quiet aria-move
    systemctl is-active --quiet aria2-existing
    sha256sum -c /root/config.sha256
    cmp /root/original-secret /var/lib/aria-move/rpc-secret
    download
    systemctl disable --now aria2-existing
    rm /etc/systemd/system/aria2-existing.service /var/lib/aria-move/existing.conf
    systemctl daemon-reload
    systemctl enable --now aria-move
    active
    apt-get remove -y aria-move
    ! systemctl is-active --quiet aria-move
    ! systemctl is-enabled --quiet aria-move
    test -f /etc/aria_move/config.xml
    test -f /var/lib/aria-move/completed/retained.txt
    apt-get install -y "$package"
    active
    sha256sum -c /root/config.sha256
    apt-get purge -y aria-move
    test ! -e /etc/aria_move/config.xml
    test ! -e /etc/aria_move/aria2.conf
    test ! -e /usr/bin/aria_move
    test -f /var/lib/aria-move/completed/retained.txt
    apt-get install -y "$package"
    active
    cmp /root/original-secret /var/lib/aria-move/rpc-secret
    download
    apt-get purge -y aria-move
    echo 'PASS: install, hook, restart, upgrade, reboot, remove, reinstall and purge'
    ;;
*) exit 2;;
esac
