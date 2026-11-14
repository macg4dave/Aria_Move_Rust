#!/bin/bash
# PID 1 in a disposable QEMU guest. Only the three attached test disks are formatted.
set -euo pipefail
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
finish() {
    result=$?
    if [ "$result" != 0 ]; then echo "ARIA_MOVE_VM_FAIL exit=$result"; fi
    sync
    poweroff -f
}
trap finish EXIT
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev || true
mkdir -p /run /tmp /mnt/{ext4,xfs,btrfs,smb}
mount -t tmpfs tmpfs /run
mount -t tmpfs tmpfs /tmp
echo "Debian guest: $(cat /etc/debian_version); kernel: $(uname -r)"
for disk in /dev/vdb /dev/vdc /dev/vdd; do
    test -b "$disk"
done
mkfs.ext4 -q -F /dev/vdb
mkfs.xfs -q -f /dev/vdc
mkfs.btrfs -q -f /dev/vdd
mount /dev/vdb /mnt/ext4
mount /dev/vdc /mnt/xfs
mount /dev/vdd /mnt/btrfs
# A real Linux SMB client and server in the guest; no production credentials.
mkdir -p /mnt/ext4/smb-export /run/samba
chmod 0777 /mnt/ext4/smb-export
cat > /tmp/smb.conf <<'EOF'
[global]
    server role = standalone server
    security = user
    map to guest = Bad User
    interfaces = 127.0.0.1
    bind interfaces only = yes
    smb ports = 445
    log file = /tmp/samba.log
[test]
    path = /mnt/ext4/smb-export
    read only = no
    guest ok = yes
    force user = root
EOF
ip link set lo up
smbd -D -s /tmp/smb.conf
mount -t cifs //127.0.0.1/test /mnt/smb -o guest,vers=3.1.1,cache=strict,actimeo=1
findmnt -t ext4,xfs,btrfs,cifs -o TARGET,SOURCE,FSTYPE
python3 /moves.py
echo ARIA_MOVE_VM_PASS
