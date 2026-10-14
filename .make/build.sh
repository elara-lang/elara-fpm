#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=elara-env.sh
source "$SCRIPT_DIR/elara-env.sh"
elara_resolve

ROOT_DIR="$(cd "$SCRIPT_DIR/../elara_fpm" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_DIR="${BUILD_DIR:-$REPO_ROOT/bin}"
BINARY_NAME="${BINARY_NAME:-elara-fpm}"
CARGO="${CARGO:-cargo}"

export ELARA_INCLUDE ELARA_LIBDIR ELARA_LIB ELARA_MODE
export LD_LIBRARY_PATH="${ELARA_LIBDIR}${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

mkdir -p "$BUILD_DIR"
echo "Building $BINARY_NAME (Elara mode=${ELARA_MODE})..."
$CARGO build --release --manifest-path "$ROOT_DIR/Cargo.toml" --bin "$BINARY_NAME"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/target}"
install -m 755 "$TARGET_DIR/release/$BINARY_NAME" "$BUILD_DIR/$BINARY_NAME"
echo "Done: $BUILD_DIR/$BINARY_NAME"
