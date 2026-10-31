#!/usr/bin/env bash
set -euo pipefail
# Run on macOS with both Rust targets installed. No network upload is performed.
[ "$(uname -s)" = Darwin ] || { echo 'macOS and lipo are required.' >&2; exit 1; }
cargo build --locked --release --target x86_64-apple-darwin
cargo build --locked --release --target aarch64-apple-darwin
OUTPUT="target/universal2-apple-darwin/release"
mkdir -p "$OUTPUT"
lipo -create target/x86_64-apple-darwin/release/aria_move target/aarch64-apple-darwin/release/aria_move -output "$OUTPUT/aria_move"
lipo -verify_arch x86_64 arm64 "$OUTPUT/aria_move"
tar -czf target/aria_move-universal2-apple-darwin.tar.gz -C "$OUTPUT" aria_move -C "$(pwd)" LICENSE README.md
shasum -a 256 target/aria_move-universal2-apple-darwin.tar.gz > target/aria_move-universal2-apple-darwin.tar.gz.sha256
