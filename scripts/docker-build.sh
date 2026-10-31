#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
case "${1:---deb}" in --deb) ;; *) echo 'Usage: scripts/docker-build.sh [--deb]' >&2; exit 2;; esac
mkdir -p "$ROOT/target"
docker build -f "$ROOT/docker/Dockerfile.build" -t aria-move-build "$ROOT"
docker run --rm --mount "type=bind,source=$ROOT,target=/source,readonly" \
  --mount "type=bind,source=$ROOT/target,target=/out" aria-move-build bash -ec '
  mkdir -p /work/source
  tar -C /source --exclude=target --exclude=.git -cf - . | tar -C /work/source -xf -
  cd /work/source
  bash scripts/build-linux-release.sh
  mkdir -p /out/debian
  cp /work/*.deb /out/debian/
'
