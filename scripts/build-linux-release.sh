#!/usr/bin/env bash
# Run inside a native-architecture Debian Rust container with sources at /work
# and the host artifact directory mounted at /out.
set -euo pipefail
export RUSTUP_TOOLCHAIN="$(cd / && rustup show active-toolchain | cut -d ' ' -f 1)"
cd /work
apt-get update -qq
apt-get install -y -qq --no-install-recommends rpm zip
version=$(sed -n 's/^version = "\([^"]*\)".*/\1/p' Cargo.toml | head -1)
arch=$(uname -m)
deb_arch=$(dpkg --print-architecture)
cargo build --locked --release --all-features
binary=/work/target/release/aria_move
"$binary" --version
readelf -h "$binary" | sed -n '/Machine:/p'

stage=$(mktemp -d)
mkdir -p "$stage/zip/bin" "$stage/deb/DEBIAN" "$stage/deb/usr/bin" \
  "$stage/deb/usr/share/doc/aria-move" "$stage/deb/usr/share/licenses/aria-move"
install -m755 "$binary" "$stage/zip/bin/aria_move"
cp LICENSE README.md "$stage/zip/"
(cd "$stage/zip" && zip -q -r "/out/aria_move-${version}-linux-${arch}.zip" .)

install -m755 "$binary" "$stage/deb/usr/bin/aria_move"
cp README.md "$stage/deb/usr/share/doc/aria-move/"
cp LICENSE "$stage/deb/usr/share/licenses/aria-move/"
cat > "$stage/deb/DEBIAN/control" <<EOF
Package: aria-move
Version: $version
Section: utils
Priority: optional
Architecture: $deb_arch
Maintainer: Dave <macg4dave@users.noreply.github.com>
Depends: libc6 (>= 2.36), libgcc-s1 (>= 3.0)
Homepage: https://github.com/macg4dave/Aria_Move_Rust
Description: Safely move completed aria2 downloads
 Verified cross-filesystem copies, exclusive publication and metadata support.
EOF
dpkg-deb --root-owner-group --build "$stage/deb" "/out/aria-move_${version}_${deb_arch}.deb"

mkdir -p "$stage/rpm/SPECS"
cat > "$stage/rpm/SPECS/aria-move.spec" <<EOF
%global debug_package %{nil}
%global _build_id_links none
Name: aria-move
Version: $version
Release: 1
Summary: Safely move completed aria2 downloads
License: MIT
URL: https://github.com/macg4dave/Aria_Move_Rust
Requires: glibc >= 2.36

%description
Verified cross-filesystem copies, exclusive publication and metadata support.

%install
install -D -m755 $binary %{buildroot}/usr/bin/aria_move
install -D -m644 /work/LICENSE %{buildroot}%{_licensedir}/aria-move/LICENSE
install -D -m644 /work/README.md %{buildroot}%{_docdir}/aria-move/README.md

%files
/usr/bin/aria_move
%license %{_licensedir}/aria-move/LICENSE
%doc %{_docdir}/aria-move/README.md
EOF
rpmbuild --define "_topdir $stage/rpm" -bb "$stage/rpm/SPECS/aria-move.spec"
cp "$stage/rpm/RPMS/$arch/aria-move-${version}-1.${arch}.rpm" /out/

# Verify installation, architecture and executable startup in this disposable
# container. rpm --nodeps is necessary because Debian tracks libraries via dpkg.
dpkg -i "/out/aria-move_${version}_${deb_arch}.deb"
test "$(aria_move --version)" = "aria_move $version"
dpkg -r aria-move
rpm -qp --requires "/out/aria-move-${version}-1.${arch}.rpm"
rpm -i --nodeps "/out/aria-move-${version}-1.${arch}.rpm"
test "$(aria_move --version)" = "aria_move $version"
rpm -e aria-move
unzip -t "/out/aria_move-${version}-linux-${arch}.zip"
