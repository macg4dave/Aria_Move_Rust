#!/usr/bin/env bash
set -euo pipefail

# Build (debug) + test the project inside a Debian-based Rust container so produced binaries
# are linked against Debian glibc and system libraries. Run from repo root.
# By default this performs a debug build and runs all tests.
# Use --deb to additionally build a release binary and produce a .deb.
# Usage: ./scripts/docker-build.sh [--deb]

ROOT=$(pwd)
IMAGE_TAG="aria_move_build:1.76-bullseye"
DOCKERFILE="docker/Dockerfile.build"
DEB_OUTPUT_DIR="$ROOT/target/debian"

REBUILD=false

MAKE_DEB=false
for arg in "$@"; do
  case "$arg" in
    --deb) MAKE_DEB=true ;;
    --rebuild) REBUILD=true ;;
    *) ;;
  esac
done

# Ensure output dir exists on host
mkdir -p "$DEB_OUTPUT_DIR"

# Build (or reuse cached) image that already has dependencies fetched.
if [ "$REBUILD" = true ]; then
  echo "[docker-build] Rebuilding image without cache..."
  docker build --no-cache -f "$DOCKERFILE" -t "$IMAGE_TAG" .
elif ! docker image inspect "$IMAGE_TAG" >/dev/null 2>&1; then
  echo "[docker-build] Building image $IMAGE_TAG (first time)..."
  docker build -f "$DOCKERFILE" -t "$IMAGE_TAG" .
else
  echo "[docker-build] Using cached image $IMAGE_TAG"
fi

# Mount host Cargo cache for even faster incremental builds (optional but helpful).
CARGO_HOME_HOST="${CARGO_HOME:-$HOME/.cargo}"
mkdir -p "$CARGO_HOME_HOST/registry" "$CARGO_HOME_HOST/git"

docker run --rm \
  -e DEBIAN_FRONTEND=noninteractive \
  -e MAKE_DEB="$MAKE_DEB" \
  -v "$ROOT":/work \
  -v "$CARGO_HOME_HOST/registry":/usr/local/cargo/registry \
  -v "$CARGO_HOME_HOST/git":/usr/local/cargo/git \
  -w /work "$IMAGE_TAG" \
  bash /work/scripts/docker-build-in-container.sh

if [ "$MAKE_DEB" = true ]; then
  echo "Debian packages (if produced) are in: $DEB_OUTPUT_DIR"
fi
echo "Debug build output is in: target/debug/"
echo "Tests executed inside container (dependencies cached)."

exit 0
