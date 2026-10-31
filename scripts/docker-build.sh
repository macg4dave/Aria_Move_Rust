#!/usr/bin/env bash
# Build a release binary (and, with --deb, a Debian package) inside a Debian
# container so it links against Debian's glibc. Output lands in the host's
# target/ directory. Tests run separately: scripts/test-linux.sh.
#
#   scripts/docker-build.sh [--deb] [--rebuild]
set -euo pipefail

IMAGE_TAG="aria_move_build:1.99-bookworm"
DOCKERFILE="docker/Dockerfile.build"
ROOT="$(cd "$(dirname "$0")/.." && (pwd -W 2>/dev/null || pwd))"
MAKE_DEB=false
REBUILD=false
for arg in "$@"; do
  case "$arg" in
    --deb) MAKE_DEB=true ;;
    --rebuild) REBUILD=true ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done

if [ "$REBUILD" = true ] || ! docker image inspect "$IMAGE_TAG" >/dev/null 2>&1; then
  docker build ${REBUILD:+$([ "$REBUILD" = true ] && echo --no-cache)} -f "$DOCKERFILE" -t "$IMAGE_TAG" "$ROOT"
fi

MSYS_NO_PATHCONV=1 docker run --rm \
  -e MAKE_DEB="$MAKE_DEB" \
  --mount "type=bind,source=$ROOT,target=/work" \
  --mount "type=volume,source=aria_move_registry_amd64,target=/usr/local/cargo/registry" \
  -w /work "$IMAGE_TAG" bash -euo pipefail -c '
    cargo build --release --locked
    if [ "$MAKE_DEB" = true ]; then
      command -v cargo-deb >/dev/null || cargo install cargo-deb --locked
      cargo deb --no-build --no-strip
    fi
  '
echo "Release binary: target/release/aria_move"
[ "$MAKE_DEB" = true ] && echo "Debian package: target/debian/"
exit 0
