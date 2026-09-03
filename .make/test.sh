#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=elara-env.sh
source "$SCRIPT_DIR/elara-env.sh"
elara_resolve

ROOT_DIR="$(cd "$SCRIPT_DIR/../elara_http" && pwd)"
CARGO="${CARGO:-cargo}"
export ELARA_INCLUDE ELARA_LIBDIR ELARA_LIB ELARA_MODE
export LD_LIBRARY_PATH="${ELARA_LIBDIR}${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

$CARGO test --manifest-path "$ROOT_DIR/Cargo.toml"
