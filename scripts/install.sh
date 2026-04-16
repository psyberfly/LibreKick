#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/source_env.sh"

TARGET_NAME="${1:-${TARGET:-}}"

if [[ -z "$TARGET_NAME" ]]; then
  echo "target is not set. Define TARGET in scripts/config.env (e.g. TARGET=linux)."
  exit 1
fi

if [[ ! -f "$BINARY_SRC" ]]; then
  echo "Release binary not found. Running full build first..."
  "$SCRIPT_DIR/build.sh" "$TARGET_NAME"
elif [[ ! -f "$BINARY_DST" ]] || [[ "$BINARY_SRC" -nt "$BINARY_DST" ]]; then
  echo "Bundled plugin is missing or stale. Refreshing bundle first..."
  "$SCRIPT_DIR/core/bundle.sh" "$TARGET_NAME"
fi

"$SCRIPT_DIR/core/install.sh" "$TARGET_NAME"
