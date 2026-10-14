#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "$0")/../elara_fpm" && pwd)"
CARGO="${CARGO:-cargo}"
$CARGO fmt --manifest-path "$ROOT_DIR/Cargo.toml" --all
