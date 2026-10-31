#!/usr/bin/env bash
# Build and test the exact release packages. Requires a local Linux Docker engine.
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && (pwd -W 2>/dev/null || pwd))
case $(uname -s) in MINGW*|MSYS*) export MSYS_NO_PATHCONV=1;; esac
arches=("$@")
[[ ${#arches[@]} -gt 0 ]] || arches=(amd64 arm64)
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repo/Cargo.toml" | head -1)
out="$repo/target/release-check"
mkdir -p "$out"
git -C "$repo" rev-parse HEAD > "$out/source-commit.txt"
git -C "$repo" status --porcelain > "$out/source-status.txt"
guest=
cleanup() {
    local status=$?
    if [[ -n $guest ]]; then
        docker logs "$guest" > "$out/$arch-boot.log" 2>&1 || true
        docker rm -f "$guest" >/dev/null 2>&1 || true
    fi
    if [[ $status != 0 ]]; then echo "Release gate failed; inspect logs in $out" >&2; fi
}
trap cleanup EXIT
for arch in "${arches[@]}"; do
    case $arch in amd64|arm64) ;; *) echo "Unsupported architecture: $arch" >&2; exit 2;; esac
    platform="linux/$arch"
    rm -f "$out/$arch-PASS"
    docker build --platform "$platform" -f "$repo/docker/Dockerfile.build" -t "aria-move-build:$arch" "$repo" > "$out/$arch-image.log" 2>&1
    docker run --rm --platform "$platform" \
      --mount "type=bind,source=$repo,target=/source,readonly" \
      --mount "type=bind,source=$out,target=/out" \
      --mount "type=volume,source=aria_move_release_registry_$arch,target=/usr/local/cargo/registry" \
      --mount "type=volume,source=aria_move_release_target_$arch,target=/work/source/target" \
      "aria-move-build:$arch" bash -euc '
        export RUSTUP_TOOLCHAIN=$(cd / && rustup show active-toolchain | cut -d " " -f 1)
        mkdir -p /work/source
        tar -C /source --exclude=target --exclude=.git -cf - . | tar -C /work/source -xf -
        cd /work/source
        useradd --create-home tester
        chown -R tester /work
        chmod -R a+rwX /usr/local/cargo
        runuser -u tester --preserve-environment -- env HOME=/home/tester bash -euc "
          cargo fmt --all -- --check
          cargo clippy --locked --all-targets --all-features -- -D warnings
          bash scripts/build-linux-release.sh
        "
        runuser -u tester -- lintian ../*.deb
        cp ../*.deb /out/
      ' > "$out/$arch-build.log" 2>&1
    package="aria-move_${version}_${arch}.deb"
    sha256sum "$out/$package" | sed "s|$out/||" > "$out/$package.sha256"
    docker build --platform "$platform" -f "$repo/docker/Dockerfile.release-test" -t "aria-move-release-test:$arch" "$repo" > "$out/$arch-test-image.log" 2>&1
    guest="aria-move-release-$arch-$$"
    # Privilege is confined to this disposable test container. Never mount host
    # storage or a host Docker socket writable into it.
    docker run -d --name "$guest" --platform "$platform" --privileged \
      --cgroupns=private --tmpfs /run --tmpfs /run/lock \
      --mount "type=bind,source=$repo/tests/packaging,target=/package-tests,readonly" \
      "aria-move-release-test:$arch" > /dev/null
    docker exec "$guest" mkdir -p /packages
    docker cp "$out/$package" "$guest:/packages/package.deb"
    ready() {
        for ((i=0; i<90; i++)); do
            # Container kernels can leave unrelated device/sysctl units degraded.
            # Wait for the normal boot target; test our service strictly below.
            if docker exec "$guest" systemctl is-active --quiet multi-user.target; then return; fi
            sleep 1
        done
        docker logs "$guest"
        return 1
    }
    ready
    url=${ARIA_MOVE_TEST_URL:-https://deb.debian.org/debian/README}
    docker exec -e "ARIA_MOVE_TEST_URL=$url" "$guest" bash /package-tests/lifecycle.sh before-reboot > "$out/$arch-lifecycle.log" 2>&1
    # A Docker restart tests systemd startup in the same installed container.
    # A native host reboot remains an additional hardware release check.
    docker restart "$guest" >/dev/null
    ready
    docker exec -e "ARIA_MOVE_TEST_URL=$url" "$guest" bash /package-tests/lifecycle.sh after-reboot >> "$out/$arch-lifecycle.log" 2>&1
    docker exec "$guest" dpkg --audit >> "$out/$arch-lifecycle.log"
    printf '%s\n' "PASS $package" > "$out/$arch-PASS"
    cleanup
    guest=
done
echo "Verified packages, checksums and logs: $out"
