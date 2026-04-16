#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/source_env.sh"

if [[ ! -d "$LOCAL_VST3_DIR" ]]; then
  echo "VST path does not exist: $LOCAL_VST3_DIR"
  echo "Run: bash $ROOT_DIR/scripts/compile_linux.sh"
  exit 1
fi

if [[ ! -d "$PLUGIN_BUNDLE_DIR" ]]; then
  echo "Plugin bundle not found: $PLUGIN_BUNDLE_DIR"
  echo "Run: bash $ROOT_DIR/scripts/compile_linux.sh"
  exit 1
fi

export VST3_PATH="$LOCAL_VST3_DIR:${VST3_PATH:-}"
export CARLA_PLUGIN_PATH="$LOCAL_VST3_DIR:${CARLA_PLUGIN_PATH:-}"

if command -v carla-single >/dev/null 2>&1; then
  echo "Starting Carla Single with: $PLUGIN_BUNDLE_DIR"
  exec carla-single vst3 "$PLUGIN_BUNDLE_DIR"
fi

if command -v carla >/dev/null 2>&1; then
  echo "carla-single not found; starting Carla host with VST path: $LOCAL_VST3_DIR"
  exec carla
elif command -v carla2 >/dev/null 2>&1; then
  echo "carla-single not found; starting Carla host with VST path: $LOCAL_VST3_DIR"
  exec carla2
else
  echo "Carla is not installed or not in PATH."
  echo "Install Carla and retry."
  exit 1
fi
