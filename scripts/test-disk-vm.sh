#!/bin/bash
# Full Debian 13 VM; QEMU TCG works without Hyper-V/KVM access.
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && (pwd -W 2>/dev/null || pwd))
case $(uname -s) in MINGW*|MSYS*) export MSYS_NO_PATHCONV=1;; esac
out="$repo/target/disk-vm"
mkdir -p "$out/results"
docker build -f "$repo/docker/Dockerfile.disk-vm" -t aria-move-disk-vm:debian13 "$repo" > "$out/build.log" 2>&1
guest="aria-move-disk-vm-$$"
cleanup() {
    docker cp "$guest:/work/serial.log" "$out/serial.log" 2>/dev/null || true
    docker cp "$guest:/work/export/results/." "$out/results/" 2>/dev/null || true
    docker rm -f "$guest" >/dev/null 2>&1 || true
}
trap cleanup EXIT
docker run --name "$guest" aria-move-disk-vm:debian13 > "$out/console.log" 2>&1
docker cp "$guest:/work/serial.log" "$out/serial.log"
mkdir -p "$out/results"
docker cp "$guest:/work/export/results/." "$out/results/"
test -s "$out/results/report.json"
grep -q '^ARIA_MOVE_VM_PASS' "$out/serial.log"
echo "PASS: Debian 13 virtual-disk matrix. Logs: $out"
