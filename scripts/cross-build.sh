#!/usr/bin/env bash
set -euo pipefail

# Wrapper to build Linux targets using `cross`.
# Requires Docker and QEMU (cross will handle those).
# Usage examples:
#  ./scripts/cross-build.sh x86_64-unknown-linux-gnu
#  ./scripts/cross-build.sh x86_64-unknown-linux-musl --release

TARGET=${1:-x86_64-unknown-linux-gnu}
shift || true
RELEASE=false
EXTRA_ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) RELEASE=true; shift ;;
    *) EXTRA_ARGS+=("$1"); shift ;;
  esac
done

# Ensure `cross` is installed
if ! command -v cross >/dev/null 2>&1; then
  echo "'cross' not found; installing via cargo (requires a functioning Rust toolchain)"
  cargo install cross --locked
fi

if [ "$RELEASE" = true ]; then
  cross build --target "$TARGET" --release "${EXTRA_ARGS[@]}"
else
  cross build --target "$TARGET" "${EXTRA_ARGS[@]}"
fi

echo "Built target: $TARGET"
if [ "$RELEASE" = true ]; then
  echo "Binary: target/$TARGET/release/$(basename $(pwd))"
else
  echo "Binary: target/$TARGET/debug/$(basename $(pwd))"
fi
