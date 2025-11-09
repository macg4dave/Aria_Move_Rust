#!/usr/bin/env bash
set -euo pipefail

# Build the project inside a Debian-based Rust container so produced binaries
# are linked against Debian glibc and system libraries. Run from repo root.
# Usage: ./scripts/docker-build.sh [--deb]

ROOT=$(pwd)
RUST_IMAGE="rust:1.76-bullseye"
DEB_OUTPUT_DIR="$ROOT/target/debian"

MAKE_DEB=false
for arg in "$@"; do
  case "$arg" in
    --deb) MAKE_DEB=true ;;
    *) ;;
  esac
done

# Ensure output dir exists on host
mkdir -p "$DEB_OUTPUT_DIR"

docker run --rm -e DEBIAN_FRONTEND=noninteractive -e MAKE_DEB="$MAKE_DEB" -v "$ROOT":/work -w /work "$RUST_IMAGE" \
  bash /work/scripts/docker-build-in-container.sh

if [ "$MAKE_DEB" = true ]; then
  echo "Debian packages (if produced) are in: $DEB_OUTPUT_DIR"
else
  echo "Built release binary in: target/release/"
fi

exit 0
