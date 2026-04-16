#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../source_env.sh"

TARGET="${1:-${TARGET:-}}"

if [[ -z "$TARGET" ]]; then
  echo "target is not set. Define TARGET in scripts/config.env (e.g. TARGET=linux)."
  exit 1
fi

if [[ ! -f "$BINARY_SRC" ]]; then
  echo "Missing binary: $BINARY_SRC"
  echo "Run: bash $ROOT_DIR/scripts/build.sh ${TARGET}"
  exit 1
fi

case "$TARGET" in
  linux)
    rm -rf "$PLUGIN_BUNDLE_DIR"
    mkdir -p "$BINARY_DST_DIR"
    cp "$BINARY_SRC" "$BINARY_DST"
    echo "Created: $PLUGIN_BUNDLE_DIR"
    ;;
  *)
    echo "Unsupported target: $TARGET"
    echo "Currently supported: linux"
    exit 1
    ;;
esac

