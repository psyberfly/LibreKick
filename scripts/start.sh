#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/source_env.sh"

MODE="${1:-single}"
TARGET_NAME="${2:-${TARGET:-}}"

if [[ "$MODE" != "single" && "$MODE" != "full" ]]; then
  echo "Usage: $0 [single|full] [target]"
  exit 1
fi

if [[ -z "$TARGET_NAME" ]]; then
  echo "target is not set. Define TARGET in scripts/config.env (e.g. TARGET=linux)."
  exit 1
fi

"$SCRIPT_DIR/core/start.sh" "$TARGET_NAME" "$MODE"
