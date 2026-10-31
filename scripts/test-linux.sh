#!/usr/bin/env bash
# Run the full Linux gate (fmt, clippy, tests; with and without the xattrs
# feature) inside Docker for each requested architecture.
#
#   scripts/test-linux.sh            # amd64 and arm64
#   scripts/test-linux.sh arm64      # one architecture
#
# arm64 runs natively on ARM hosts and under QEMU emulation elsewhere (Docker
# Desktop includes it; on Linux install qemu-user-static/binfmt). Sources are
# copied into the container, so the host tree is never modified, and the tests
# run as an unprivileged user, so permission tests are not skipped as root.
# Works from Linux, macOS and Git Bash on Windows.
set -euo pipefail

IMAGE="${ARIA_MOVE_TEST_IMAGE:-rust:1.99-bookworm}"
ROOT="$(cd "$(dirname "$0")/.." && (pwd -W 2>/dev/null || pwd))"
ARCHES=("$@")
[ ${#ARCHES[@]} -eq 0 ] && ARCHES=(amd64 arm64)

for arch in "${ARCHES[@]}"; do
  case "$arch" in amd64|arm64) ;; *) echo "unknown architecture: $arch" >&2; exit 2 ;; esac
  echo "== linux/$arch =="
  MSYS_NO_PATHCONV=1 docker run --rm --platform "linux/$arch" \
    --mount "type=bind,source=$ROOT,target=/src,readonly" \
    --mount "type=volume,source=aria_move_registry_$arch,target=/usr/local/cargo/registry" \
    "$IMAGE" bash -euo pipefail -c '
      rustup component add rustfmt clippy >/dev/null 2>&1
      chmod -R a+rwX /usr/local/cargo
      useradd --create-home --shell /bin/bash tester
      mkdir /home/tester/work
      tar -C /src --exclude=./target --exclude=./.git -cf - . | tar -C /home/tester/work -xf -
      chown -R tester /home/tester/work
      su tester -c "
        set -euo pipefail
        export PATH=/usr/local/cargo/bin:\$PATH CARGO_HOME=/usr/local/cargo RUSTUP_HOME=/usr/local/rustup
        cd ~/work
        echo \"arch: \$(uname -m), uid: \$(id -u)\"
        cargo fmt --all -- --check
        cargo clippy --locked --all-targets -- -D warnings
        cargo clippy --locked --all-targets --all-features -- -D warnings
        cargo test --locked --all-targets
        cargo test --locked --all-targets --all-features
      "
    '
done
