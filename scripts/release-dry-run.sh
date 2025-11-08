#!/usr/bin/env bash
set -euo pipefail

# Dry-run end-to-end release: checks, build, dist plan/build (no upload)
# Requires: stable toolchain, cargo-dist installed (will install if missing)

echo "==> fmt check"
cargo fmt --all -- --check || true

echo "==> clippy (warnings allowed)"
cargo clippy --all-targets --all-features || true

echo "==> tests"
cargo test --all --no-fail-fast

if ! command -v cargo-dist >/dev/null 2>&1; then
  echo "==> installing cargo-dist"
  cargo install cargo-dist --locked
fi

TAG="v0.0.0-dryrun"
echo "==> cargo dist plan ($TAG)"
cargo dist plan --tag "$TAG" --artifacts=all --output-format=json > dist-plan.json

echo "==> cargo dist build ($TAG)"
cargo dist build --tag "$TAG" --artifacts=all

echo "Done. Artifacts are in target/dist (not uploaded)."
