#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "$0")/../elara_http" && pwd)"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CARGO="${CARGO:-cargo}"
$CARGO clean --manifest-path "$ROOT_DIR/Cargo.toml"
find "$REPO_ROOT/bin" -mindepth 1 ! -name '.gitkeep' -exec rm -rf {} +
