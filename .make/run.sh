#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=elara-env.sh
source "$SCRIPT_DIR/elara-env.sh"
elara_resolve

REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_DIR="${BUILD_DIR:-$REPO_ROOT/bin}"
BINARY_NAME="${BINARY_NAME:-elara-http}"

export ELARA_INCLUDE ELARA_LIBDIR ELARA_LIB ELARA_MODE
export LD_LIBRARY_PATH="${ELARA_LIBDIR}${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

if [ ! -x "$BUILD_DIR/$BINARY_NAME" ]; then
	echo "Binary not found. Run 'make build' first." >&2
	exit 1
fi

# make -C elara_http would otherwise resolve relative -t paths against elara_http/
cd "$REPO_ROOT"
exec "$BUILD_DIR/$BINARY_NAME" "$@"
