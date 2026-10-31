#!/usr/bin/env bash
set -euo pipefail
# Native local checks and artifact build. Never upload or publish.
FEATURE_ARGS=()
case "$(uname -s)" in Linux|Darwin) FEATURE_ARGS+=(--all-features);; esac
cargo fmt --all -- --check
cargo clippy --all-targets "${FEATURE_ARGS[@]}" -- -D warnings
cargo test --all --no-fail-fast "${FEATURE_ARGS[@]}"
cargo build --release --locked
command -v dist >/dev/null || { echo 'Install cargo-dist to build distribution artifacts.' >&2; exit 1; }
HOST_TARGET="$(rustc -vV | sed -n 's/^host: //p')"
TAG="v$(cargo pkgid --locked | sed 's/.*[@#]//')"
dist manifest --tag "$TAG" --artifacts=local --target "$HOST_TARGET"
dist build --tag "$TAG" --artifacts=local --target "$HOST_TARGET"
echo 'Local artifacts are in target/distrib (not uploaded).'
