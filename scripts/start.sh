#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/source_env.sh"

MODE="single"
TARGET_NAME=""
FORMAT_NAME="${FORMAT:-clap3}"

if [[ "${1:-}" == "single" || "${1:-}" == "full" ]]; then
  MODE="$1"
  TARGET_NAME="${2:-${TARGET:-}}"
  FORMAT_NAME="${3:-${FORMAT:-clap3}}"
else
  TARGET_NAME="${1:-${TARGET:-}}"
  FORMAT_NAME="${2:-${FORMAT:-clap3}}"

  if [[ "${3:-}" == "single" || "${3:-}" == "full" ]]; then
    MODE="$3"
  fi
fi

if ! FORMAT_NAME="$(normalize_format "$FORMAT_NAME")"; then
  echo "Unsupported format: $FORMAT_NAME"
  echo "Supported formats: vst3, clap3, au"
  exit 1
fi

if [[ "$MODE" != "single" && "$MODE" != "full" ]]; then
  echo "Usage: $0 [single|full] [target] [clap3|vst3|au]"
  echo "   or: $0 [target] [clap3|vst3|au] [single|full]"
  exit 1
fi

if [[ -z "$TARGET_NAME" ]]; then
  echo "target is not set. Define TARGETS/TARGET in scripts/config.env (e.g. TARGET=linux)."
  exit 1
fi

if ! is_supported_target "$TARGET_NAME"; then
  echo "Unsupported target: $TARGET_NAME"
  echo "Supported targets: linux, darwin, windows"
  exit 1
fi

PLUGIN_ARTIFACT="$(bundle_artifact_path_for "$TARGET_NAME" "$FORMAT_NAME")"
PLUGIN_BINARY="$(bundle_binary_path_for "$TARGET_NAME" "$FORMAT_NAME")"
SOURCE_BINARY="$(cargo_binary_path_for_target "$TARGET_NAME")"

if [[ ! -f "$SOURCE_BINARY" ]]; then
  echo "Release binary not found. Running full build first..."
  "$SCRIPT_DIR/build.sh" "$TARGET_NAME" "$FORMAT_NAME"
elif [[ ! -e "$PLUGIN_ARTIFACT" ]] || [[ ! -e "$PLUGIN_BINARY" ]] || [[ "$SOURCE_BINARY" -nt "$PLUGIN_BINARY" ]]; then
  echo "Bundled plugin is missing or stale. Refreshing bundle first..."
  "$SCRIPT_DIR/build.sh" "$TARGET_NAME" "$FORMAT_NAME"
fi

"$SCRIPT_DIR/core/start.sh" "$TARGET_NAME" "$MODE" "$FORMAT_NAME"
