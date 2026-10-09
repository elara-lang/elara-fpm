#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../elara_http" && pwd)"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="${BUILD_DIR:-$REPO_ROOT/bin}"
CARGO="${CARGO:-cargo}"

$CARGO clean --manifest-path "$ROOT_DIR/Cargo.toml"
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"
touch "$BUILD_DIR/.gitkeep"
