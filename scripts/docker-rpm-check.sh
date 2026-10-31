#!/usr/bin/env bash
# Build and test the exact release RPMs. Requires a local Linux Docker engine.
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && (pwd -W 2>/dev/null || pwd))
case $(uname -s) in MINGW*|MSYS*) export MSYS_NO_PATHCONV=1;; esac
arches=("$@")
[[ ${#arches[@]} -gt 0 ]] || arches=(x86_64 aarch64)
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repo/Cargo.toml" | head -1)
out="$repo/target/release-check"
mkdir -p "$out"
guest=
cleanup() {
    local status=$?
    if [[ -n $guest ]]; then
        docker logs "$guest" > "$out/$arch-rpm-boot.log" 2>&1 || true
        docker rm -f "$guest" >/dev/null 2>&1 || true
    fi
    if [[ $status != 0 ]]; then echo "RPM gate failed; inspect logs in $out" >&2; fi
}
trap cleanup EXIT
for arch in "${arches[@]}"; do
    case $arch in x86_64) platform=linux/amd64;; aarch64) platform=linux/arm64;;
        *) echo "Unsupported architecture: $arch" >&2; exit 2;; esac
    rm -f "$out/$arch-rpm-PASS"
    docker build --platform "$platform" -f "$repo/docker/Dockerfile.rpm-build" -t "aria-move-rpm-build:$arch" "$repo" > "$out/$arch-rpm-image.log" 2>&1
    # The release RPM runs the test suite in %check; the upgrade package differs
    # only in its release number and exists to exercise upgrade scriptlets.
    docker run --rm --platform "$platform" \
      --mount "type=bind,source=$repo,target=/source,readonly" \
      --mount "type=bind,source=$out,target=/out" \
      --mount "type=volume,source=aria_move_rpm_registry_$arch,target=/usr/local/cargo/registry" \
      "aria-move-rpm-build:$arch" bash -euc '
        version=$1
        top=/work/rpmbuild
        mkdir -p "$top/SOURCES" /work/aria-move-$version
        tar -C /source --exclude=target --exclude=.git -cf - . | tar -C /work/aria-move-$version -xf -
        tar -C /work -czf "$top/SOURCES/aria-move-$version.tar.gz" aria-move-$version
        useradd --create-home tester
        chown -R tester /work
        chmod -R a+rwX /usr/local/cargo /usr/local/rustup
        runuser -u tester --preserve-environment -- env HOME=/home/tester bash -euc "
          rpmbuild --define \"_topdir $top\" --define \"pkg_version $version\" -bb /source/packaging/rpm/aria-move.spec
          rpmbuild --define \"_topdir $top\" --define \"pkg_version $version\" --define \"pkg_release 1.lifecycle1\" --nocheck -bb /source/packaging/rpm/aria-move.spec
        "
        rpm -qp --requires "$top"/RPMS/*/aria-move-$version-1.el9.*.rpm
        cp "$top"/RPMS/*/aria-move-$version-1.el9.*.rpm /out/
        cp "$top"/RPMS/*/aria-move-$version-1.lifecycle1.el9.*.rpm /out/upgrade-$(uname -m).rpm
      ' _ "$version" > "$out/$arch-rpm-build.log" 2>&1
    package="aria-move-$version-1.el9.$arch.rpm"
    sha256sum "$out/$package" | sed "s|$out/||" > "$out/$package.sha256"
    docker build --platform "$platform" -f "$repo/docker/Dockerfile.rpm-test" -t "aria-move-rpm-test:$arch" "$repo" > "$out/$arch-rpm-test-image.log" 2>&1
    guest="aria-move-rpm-$arch-$$"
    # Privilege is confined to this disposable test container.
    docker run -d --name "$guest" --platform "$platform" --privileged \
      --cgroupns=private --tmpfs /run --tmpfs /run/lock \
      --mount "type=bind,source=$repo/tests/packaging,target=/package-tests,readonly" \
      "aria-move-rpm-test:$arch" > /dev/null
    docker exec "$guest" mkdir -p /packages
    docker cp "$out/$package" "$guest:/packages/package.rpm"
    docker cp "$out/upgrade-$arch.rpm" "$guest:/packages/upgrade.rpm"
    ready() {
        for ((i=0; i<90; i++)); do
            if docker exec "$guest" systemctl is-active --quiet multi-user.target; then return; fi
            sleep 1
        done
        docker logs "$guest"
        return 1
    }
    ready
    url=${ARIA_MOVE_TEST_URL:-https://deb.debian.org/debian/README}
    docker exec -e "ARIA_MOVE_TEST_URL=$url" "$guest" bash /package-tests/rpm-lifecycle.sh before-reboot > "$out/$arch-rpm-lifecycle.log" 2>&1
    docker restart "$guest" >/dev/null
    ready
    docker exec -e "ARIA_MOVE_TEST_URL=$url" "$guest" bash /package-tests/rpm-lifecycle.sh after-reboot >> "$out/$arch-rpm-lifecycle.log" 2>&1
    cleanup
    guest=
    rm -f "$out/upgrade-$arch.rpm"
    printf '%s\n' "PASS $package" > "$out/$arch-rpm-PASS"
done
echo "Verified RPMs, checksums and logs: $out"
