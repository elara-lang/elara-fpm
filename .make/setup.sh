#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "$0")/../elara_fpm" && pwd)"
CARGO="${CARGO:-cargo}"
$CARGO fetch --manifest-path "$ROOT_DIR/Cargo.toml"
