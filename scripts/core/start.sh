#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../source_env.sh"

TARGET="${1:-${TARGET:-}}"
MODE="${2:-single}"
FORMAT="${3:-${FORMAT:-clap3}}"
FORMAT="$(normalize_format "$FORMAT")"

if [[ -z "$TARGET" ]]; then
  echo "target is not set. Define TARGET in scripts/config.env (e.g. TARGET=linux)."
  exit 1
fi

if ! is_supported_target "$TARGET"; then
  echo "Unsupported target: $TARGET"
  echo "Supported targets: linux, darwin, windows"
  exit 1
fi

if ! is_supported_format "$FORMAT"; then
  echo "Unsupported format: $FORMAT"
  echo "Supported formats: vst3, clap3, au"
  exit 1
fi

if [[ "$FORMAT" == "au" && "$TARGET" != "darwin" ]]; then
  echo "Format 'au' is only valid for target 'darwin'."
  exit 1
fi

if [[ "$MODE" != "single" && "$MODE" != "full" ]]; then
  echo "Usage: $0 <target> [single|full] [clap3|vst3|au]"
  exit 1
fi

LOCAL_PLUGIN_DIR="$(format_output_root_for "$FORMAT")"
PLUGIN_ARTIFACT="$(bundle_artifact_path_for "$TARGET" "$FORMAT")"
PLUGIN_BINARY="$(bundle_binary_path_for "$TARGET" "$FORMAT")"
CANONICAL_PLUGIN_ARTIFACT="$(canonical_bundle_artifact_path_for "$TARGET" "$FORMAT")"
CANONICAL_PLUGIN_BINARY="$(canonical_bundle_binary_path_for "$TARGET" "$FORMAT")"

if [[ ! -d "$LOCAL_PLUGIN_DIR" ]]; then
  echo "Plugin output path does not exist: $LOCAL_PLUGIN_DIR"
  echo "Run: bash $ROOT_DIR/scripts/build.sh $TARGET $FORMAT"
  exit 1
fi

if [[ ! -e "$PLUGIN_ARTIFACT" ]]; then
  echo "Plugin artifact not found: $PLUGIN_ARTIFACT"
  echo "Run: bash $ROOT_DIR/scripts/build.sh $TARGET $FORMAT"
  exit 1
fi

case "$FORMAT" in
  vst3)
    export VST3_PATH="$LOCAL_PLUGIN_DIR:${VST3_PATH:-}"
    export CARLA_PLUGIN_PATH="$LOCAL_PLUGIN_DIR:${CARLA_PLUGIN_PATH:-}"
    ;;
  clap3)
    export CLAP_PATH="$LOCAL_PLUGIN_DIR:${CLAP_PATH:-}"
    export CARLA_PLUGIN_PATH="$LOCAL_PLUGIN_DIR:${CARLA_PLUGIN_PATH:-}"
    ;;
  au)
    export CARLA_PLUGIN_PATH="$LOCAL_PLUGIN_DIR:${CARLA_PLUGIN_PATH:-}"
    ;;
esac

case "$TARGET" in
  linux)
    if [[ "$FORMAT" == "au" ]]; then
      echo "Format 'au' is not runnable on linux."
      exit 1
    fi

    CARLA_FORMAT="vst3"
    if [[ "$FORMAT" == "clap3" ]]; then
      CARLA_FORMAT="clap"
    fi

    if [[ "$MODE" == "single" ]]; then
      if command -v carla-single >/dev/null 2>&1; then
        CANDIDATES=(
          "$CANONICAL_PLUGIN_BINARY"
          "$CANONICAL_PLUGIN_ARTIFACT"
          "$PLUGIN_BINARY"
          "$PLUGIN_ARTIFACT"
          "$PLUGIN_NAME"
        )

        opened_in_single=false

        for candidate in "${CANDIDATES[@]}"; do
          if [[ -z "$candidate" ]]; then
            continue
          fi
          echo "Starting Carla Single with ${CARLA_FORMAT^^} candidate: $candidate"
          if carla-single "$CARLA_FORMAT" "$candidate"; then
            opened_in_single=true
            break
          fi
          echo "Failed to open candidate in carla-single: $candidate"
        done

        if [[ "$opened_in_single" == true ]]; then
          exit 0
        fi

        echo "Could not auto-load plugin in carla-single from target output."
        if [[ "$FORMAT" == "clap3" ]]; then
          echo "Your Carla build may not support CLAP in carla-single mode; falling back to full Carla."
        else
          echo "Falling back to full Carla mode."
        fi
      else
        echo "carla-single not found; falling back to full Carla mode."
      fi
    fi

    if command -v carla >/dev/null 2>&1; then
      echo "Starting Carla host (full mode) with plugin path: $LOCAL_PLUGIN_DIR"
      echo "Note: full Carla mode does not auto-load a single plugin from CLI; use Add Plugin inside Carla."
      exec carla
    elif command -v carla2 >/dev/null 2>&1; then
      echo "Starting Carla host (full mode) with plugin path: $LOCAL_PLUGIN_DIR"
      echo "Note: full Carla mode does not auto-load a single plugin from CLI; use Add Plugin inside Carla."
      exec carla2
    else
      echo "Carla is not installed or not in PATH."
      echo "Install Carla and retry."
      exit 1
    fi
    ;;
  darwin)
    echo "Start helper does not launch hosts automatically on darwin yet."
    echo "Built artifact: $PLUGIN_ARTIFACT"
    echo "Install path: $(install_root_for darwin "$FORMAT")"
    exit 1
    ;;
  windows)
    echo "Start helper does not launch hosts automatically on windows yet."
    echo "Built artifact: $PLUGIN_ARTIFACT"
    echo "Install path: $(install_root_for windows "$FORMAT")"
    exit 1
    ;;
  *)
    echo "Unsupported target: $TARGET"
    echo "Supported targets: linux, darwin, windows"
    exit 1
    ;;
esac
