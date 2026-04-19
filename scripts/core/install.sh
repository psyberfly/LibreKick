#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../source_env.sh"

TARGET_NAME="${1:-${TARGET:-}}"
FORMAT="${2:-${FORMAT:-clap3}}"
FORMAT="$(normalize_format "$FORMAT")"

if [[ -z "$TARGET_NAME" ]]; then
  echo "target is not set. Define TARGET in scripts/config.env (e.g. TARGET=linux)."
  exit 1
fi

if ! is_supported_target "$TARGET_NAME"; then
  echo "Unsupported target: $TARGET_NAME"
  echo "Supported targets: linux, darwin, windows"
  exit 1
fi

if ! is_supported_format "$FORMAT"; then
  echo "Unsupported format: $FORMAT"
  echo "Supported formats: vst3, clap3, au"
  exit 1
fi

if [[ "$FORMAT" == "au" && "$TARGET_NAME" != "darwin" ]]; then
  echo "Skipping install for format 'au' on target '$TARGET_NAME' (au is darwin-only)."
  exit 0
fi

PLUGIN_BUNDLE_DIR="$(bundle_artifact_path_for "$TARGET_NAME" "$FORMAT")"
INSTALL_ROOT="$(install_root_for "$TARGET_NAME" "$FORMAT")"

if [[ -z "$INSTALL_ROOT" ]]; then
  echo "No install path configured for target '$TARGET_NAME' and format '$FORMAT'."
  echo "Set the corresponding *_INSTALL_PATH_* variable in scripts/config.env."
  exit 1
fi

INSTALL_BUNDLE_DIR="$INSTALL_ROOT/$(basename "$PLUGIN_BUNDLE_DIR")"

if [[ ! -e "$PLUGIN_BUNDLE_DIR" ]]; then
  echo "Missing bundle: $PLUGIN_BUNDLE_DIR"
  echo "Run: bash $ROOT_DIR/scripts/build.sh $TARGET_NAME $FORMAT"
  exit 1
fi

mkdir -p "$INSTALL_ROOT"
rm -rf "$INSTALL_BUNDLE_DIR"

if [[ -d "$PLUGIN_BUNDLE_DIR" ]]; then
  cp -a "$PLUGIN_BUNDLE_DIR" "$INSTALL_BUNDLE_DIR"
else
  cp "$PLUGIN_BUNDLE_DIR" "$INSTALL_BUNDLE_DIR"
fi

echo "Installed: $INSTALL_BUNDLE_DIR"
echo "Install complete for target: $TARGET_NAME, format: $FORMAT"
