#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
SAMPLE_ENV="$SCRIPT_DIR/sample.env"
CONFIG_ENV="$SCRIPT_DIR/config.env"

if [[ ! -f "$SAMPLE_ENV" ]]; then
  echo "Missing sample env file: $SAMPLE_ENV"
  exit 1
fi

if [[ ! -f "$CONFIG_ENV" ]]; then
  cp "$SAMPLE_ENV" "$CONFIG_ENV"
  echo "Created $CONFIG_ENV from $SAMPLE_ENV"
  echo "Edit $CONFIG_ENV for your machine, then rerun."
  exit 1
fi

set -a
source "$CONFIG_ENV"
set +a

: "${PLUGIN_NAME:?PLUGIN_NAME must be set in config.env}"
: "${PLUGIN_BINARY_NAME:?PLUGIN_BINARY_NAME must be set in config.env}"
: "${TARGET_ARCH_DIR:?TARGET_ARCH_DIR must be set in config.env}"
: "${LOCAL_VST3_RELATIVE_PATH:?LOCAL_VST3_RELATIVE_PATH must be set in config.env}"
: "${VST_INSTALL_PATH:?VST_INSTALL_PATH must be set in config.env}"

LOCAL_VST3_DIR="$ROOT_DIR/$LOCAL_VST3_RELATIVE_PATH"
PLUGIN_BUNDLE_DIR="$LOCAL_VST3_DIR/${PLUGIN_NAME}.vst3"
BINARY_SRC="$ROOT_DIR/target/$BUILD_PROFILE/$PLUGIN_BINARY_NAME"
BINARY_DST_DIR="$PLUGIN_BUNDLE_DIR/Contents/$TARGET_ARCH_DIR"
BINARY_DST="$BINARY_DST_DIR/${PLUGIN_NAME}.so"
INSTALL_BUNDLE_DIR="$VST_INSTALL_PATH/${PLUGIN_NAME}.vst3"
