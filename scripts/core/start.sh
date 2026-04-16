#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../source_env.sh"

TARGET="${1:-${TARGET:-}}"
MODE="${2:-single}"

if [[ -z "$TARGET" ]]; then
  echo "target is not set. Define TARGET in scripts/config.env (e.g. TARGET=linux)."
  exit 1
fi

if [[ "$MODE" != "single" && "$MODE" != "full" ]]; then
  echo "Usage: $0 <target> [single|full]"
  exit 1
fi

if [[ ! -d "$LOCAL_VST3_DIR" ]]; then
  echo "VST path does not exist: $LOCAL_VST3_DIR"
  echo "Run: bash $ROOT_DIR/scripts/build.sh $TARGET"
  exit 1
fi

if [[ ! -d "$PLUGIN_BUNDLE_DIR" ]]; then
  echo "Plugin bundle not found: $PLUGIN_BUNDLE_DIR"
  echo "Run: bash $ROOT_DIR/scripts/build.sh $TARGET"
  exit 1
fi

export VST3_PATH="$LOCAL_VST3_DIR:${VST3_PATH:-}"
export CARLA_PLUGIN_PATH="$LOCAL_VST3_DIR:${CARLA_PLUGIN_PATH:-}"

case "$TARGET" in
  linux)
    if [[ "$MODE" == "single" ]]; then
      if command -v carla-single >/dev/null 2>&1; then
        echo "Starting Carla Single with: $PLUGIN_BUNDLE_DIR"
        exec carla-single vst3 "$PLUGIN_BUNDLE_DIR"
      fi

      echo "carla-single not found; falling back to full Carla mode."
    fi

    if command -v carla >/dev/null 2>&1; then
      echo "Starting Carla host (full mode) with VST path: $LOCAL_VST3_DIR"
      exec carla
    elif command -v carla2 >/dev/null 2>&1; then
      echo "Starting Carla host (full mode) with VST path: $LOCAL_VST3_DIR"
      exec carla2
    else
      echo "Carla is not installed or not in PATH."
      echo "Install Carla and retry."
      exit 1
    fi
    ;;
  *)
    echo "Unsupported target: $TARGET"
    echo "Currently supported: linux"
    exit 1
    ;;
esac
