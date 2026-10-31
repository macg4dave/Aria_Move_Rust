#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
command -v dh >/dev/null || { echo 'Install debhelper and dpkg-dev.' >&2; exit 1; }
# Windows source mounts can report every file as executable. Debhelper treats
# executable config lists as programs, so normalize packaging modes first.
find debian -maxdepth 1 -type f -exec chmod 644 {} +
chmod 755 debian/rules debian/aria-move.postinst
# Rust may be supplied by rustup, which dpkg cannot inspect.
dpkg-buildpackage -b -us -uc -d
