#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/source_env.sh"

if [[ ! -f "$BINARY_SRC" ]]; then
  echo "Missing binary: $BINARY_SRC"
  echo "Run: bash $ROOT_DIR/scripts/compile_linux.sh"
  exit 1
fi

rm -rf "$PLUGIN_BUNDLE_DIR"
mkdir -p "$BINARY_DST_DIR"
cp "$BINARY_SRC" "$BINARY_DST"

echo "Created: $PLUGIN_BUNDLE_DIR"
