#!/usr/bin/env bash
set -euo pipefail
cd /work
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo build --locked
cargo test --locked --all --all-features --no-fail-fast
if [ "${MAKE_DEB:-false}" = "true" ]; then
  command -v cargo-deb >/dev/null || cargo install cargo-deb --locked
  cargo build --release --locked
  cargo deb --no-strip --no-build
fi
