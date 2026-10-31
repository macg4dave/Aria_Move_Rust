#!/usr/bin/env bash
# Heavy integration check: real aria2 hooks and a genuine 2 GiB copy.
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && (pwd -W 2>/dev/null || pwd))
case $(uname -s) in MINGW*|MSYS*) export MSYS_NO_PATHCONV=1;; esac
arch=${1:-amd64}
case $arch in amd64|arm64) ;; *) echo 'Usage: docker-concurrency-check.sh [amd64|arm64] [PACKAGE.deb]' >&2; exit 2;; esac
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repo/Cargo.toml" | head -1)
package=${2:-$repo/target/release-check/aria-move_${version}_${arch}.deb}
test -f "$package"
guest="aria-move-concurrency-$arch-$$"
out="$repo/target/release-check"
mkdir -p "$out"
cleanup() {
    docker logs "$guest" > "$out/$arch-concurrency-boot.log" 2>&1 || true
    docker rm -f "$guest" >/dev/null 2>&1 || true
}
trap cleanup EXIT
docker build --platform "linux/$arch" -f "$repo/docker/Dockerfile.release-test" -t "aria-move-release-test:$arch" "$repo" > "$out/$arch-concurrency-image.log" 2>&1
docker run -d --name "$guest" --platform "linux/$arch" --privileged \
  --cgroupns=private --tmpfs /run --tmpfs /run/lock \
  --tmpfs /var/lib/aria-move/completed:size=3g \
  --mount "type=bind,source=$repo/tests/packaging,target=/package-tests,readonly" \
  "aria-move-release-test:$arch" >/dev/null
docker exec "$guest" mkdir -p /packages
docker cp "$package" "$guest:/packages/package.deb"
for ((i=0; i<90; i++)); do
    if docker exec "$guest" systemctl is-active --quiet multi-user.target; then break; fi
    sleep 1
done
docker exec "$guest" bash -euc 'DEBIAN_FRONTEND=noninteractive apt-get install -y /packages/package.deb; systemctl is-active --quiet aria-move' > "$out/$arch-concurrency.log" 2>&1
docker exec "$guest" python3 /package-tests/concurrency.py >> "$out/$arch-concurrency.log" 2>&1
cat "$out/$arch-concurrency.log"
