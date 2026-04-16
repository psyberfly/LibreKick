#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../source_env.sh"

TARGET="${1:-${TARGET:-}}"

if [[ -z "$TARGET" ]]; then
  echo "target is not set. Define TARGET in scripts/config.env (e.g. TARGET=linux)."
  exit 1
fi

if [[ ! -d "$PLUGIN_BUNDLE_DIR" ]]; then
  echo "Missing bundle: $PLUGIN_BUNDLE_DIR"
  echo "Run: bash $ROOT_DIR/scripts/build.sh $TARGET"
  exit 1
fi

case "$TARGET" in
  linux)
    mkdir -p "$VST_INSTALL_PATH"
    rm -rf "$INSTALL_BUNDLE_DIR"
    cp -a "$PLUGIN_BUNDLE_DIR" "$INSTALL_BUNDLE_DIR"
    echo "Installed: $INSTALL_BUNDLE_DIR"
    ;;
  *)
    echo "Unsupported target: $TARGET"
    echo "Currently supported: linux"
    exit 1
    ;;
esac
