#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/source_env.sh"

if [[ ! -d "$PLUGIN_BUNDLE_DIR" ]]; then
  echo "Missing bundle: $PLUGIN_BUNDLE_DIR"
  echo "Run: bash scripts/compile_linux.sh"
  exit 1
fi

mkdir -p "$VST_INSTALL_PATH"
rm -rf "$INSTALL_BUNDLE_DIR"
cp -a "$PLUGIN_BUNDLE_DIR" "$INSTALL_BUNDLE_DIR"

echo "Installed: $INSTALL_BUNDLE_DIR"
