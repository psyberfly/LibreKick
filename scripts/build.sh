#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/source_env.sh"

TARGET_NAME="${1:-${TARGET:-}}"

if [[ -z "$TARGET_NAME" ]]; then
  echo "target is not set. Define TARGET in scripts/config.env (e.g. TARGET=linux)."
  exit 1
fi

"$SCRIPT_DIR/core/compile.sh" "$TARGET_NAME"
"$SCRIPT_DIR/core/bundle.sh" "$TARGET_NAME"

echo "Build pipeline complete for target: $TARGET_NAME"
