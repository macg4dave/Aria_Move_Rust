#!/bin/bash
set -euo pipefail
mkdir -p /work
cd /work
qemu-img create -q -f qcow2 -F qcow2 -b /base/root.qcow2 root.qcow2
for format in ext4 xfs btrfs; do
    qemu-img create -q -f qcow2 "$format.qcow2" 1G
done
collect_results() {
    # Export guest logs even when its tests fail. This touches only the VM image.
    mkdir -p /work/export
    qemu-img convert -f qcow2 -O raw root.qcow2 /work/result.raw
    debugfs -R 'rdump /results /work/export' /work/result.raw > /work/extract.log 2>&1 || true
    rm -f /work/result.raw
}
trap collect_results EXIT
timeout --signal=TERM --kill-after=10s 900s qemu-system-x86_64 \
    -machine q35 -accel tcg -cpu max -smp 2 -m 1536 \
    -display none -serial stdio -monitor none -no-reboot \
    -kernel /base/vmlinuz -initrd /base/initrd \
    -append 'root=/dev/vda rw console=ttyS0 init=/test-init noresume panic=1' \
    -drive file=root.qcow2,format=qcow2,if=virtio \
    -drive file=ext4.qcow2,format=qcow2,if=virtio \
    -drive file=xfs.qcow2,format=qcow2,if=virtio \
    -drive file=btrfs.qcow2,format=qcow2,if=virtio \
    -nic none 2>&1 | tee /work/serial.log
grep -q '^ARIA_MOVE_VM_PASS' /work/serial.log
