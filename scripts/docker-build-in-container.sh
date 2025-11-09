#!/usr/bin/env bash
set -euo pipefail

# This script is executed *inside* the container. It expects the repository
# to be mounted at /work and an environment variable MAKE_DEB present.

cd /work

export DEBIAN_FRONTEND=noninteractive

apt-get update
apt-get install -y --no-install-recommends build-essential pkg-config libssl-dev ca-certificates gnupg debhelper

# Ensure 'cargo' is available; install rustup non-interactively if missing.
if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found in container — installing rustup toolchain (non-interactive)..."
  curl https://sh.rustup.rs -sSf | sh -s -- -y
  export PATH="$HOME/.cargo/bin:$PATH"
else
  echo "cargo present: $(cargo --version)"
fi

# Install cargo-deb if possible (non-fatal)
cargo install cargo-deb --locked || true

export PATH="$HOME/.cargo/bin:$PATH"

# Default: debug build + run tests. Optionally do a release build if packaging.

# Debug build
cargo build --manifest-path Cargo.toml

# Run full test suite (workspace-aware flags kept simple for single-crate repo)
cargo test --all-targets --manifest-path Cargo.toml

# If requested, produce a release build and a .deb package
if [ "${MAKE_DEB:-false}" = "true" ]; then
  cargo build --release --manifest-path Cargo.toml
  # cargo-deb will produce a deb under target/debian/
  cargo deb --no-strip --no-build || true
fi

echo "Container build finished (debug build + tests)."

exit 0
