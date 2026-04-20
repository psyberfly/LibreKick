#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../source_env.sh"

TARGET="${1:-${TARGET:-}}"
FORMAT="${2:-${FORMAT:-clap3}}"
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
  echo "Skipping format 'au' for target '$TARGET' (au is darwin-only)."
  exit 0
fi

BINARY_SRC="$(cargo_binary_path_for_target "$TARGET")"
BUNDLE_ARTIFACT="$(bundle_artifact_path_for "$TARGET" "$FORMAT")"
BINARY_DST="$(bundle_binary_path_for "$TARGET" "$FORMAT")"
BINARY_DST_DIR="$(dirname "$BINARY_DST")"
CANONICAL_BUNDLE_ARTIFACT="$(canonical_bundle_artifact_path_for "$TARGET" "$FORMAT")"
CANONICAL_BINARY_DST="$(canonical_bundle_binary_path_for "$TARGET" "$FORMAT")"

if [[ ! -f "$BINARY_SRC" ]]; then
  echo "Missing binary: $BINARY_SRC"
  echo "Run: bash $ROOT_DIR/scripts/build.sh ${TARGET} ${FORMAT}"
  exit 1
fi

case "$FORMAT" in
  vst3|au)
    rm -rf "$BUNDLE_ARTIFACT"
    mkdir -p "$BINARY_DST_DIR"
    cp "$BINARY_SRC" "$BINARY_DST"

    if [[ -n "$CANONICAL_BUNDLE_ARTIFACT" && "$CANONICAL_BUNDLE_ARTIFACT" != "$BUNDLE_ARTIFACT" ]]; then
      rm -rf "$CANONICAL_BUNDLE_ARTIFACT"
      mkdir -p "$(dirname "$CANONICAL_BINARY_DST")"
      cp "$BINARY_SRC" "$CANONICAL_BINARY_DST"
      echo "Created (compat): $CANONICAL_BUNDLE_ARTIFACT"
    fi
    ;;
  clap3)
    mkdir -p "$(dirname "$BUNDLE_ARTIFACT")"
    rm -f "$BUNDLE_ARTIFACT"
    cp "$BINARY_SRC" "$BUNDLE_ARTIFACT"
    chmod +x "$BUNDLE_ARTIFACT"

    if [[ -n "$CANONICAL_BUNDLE_ARTIFACT" && "$CANONICAL_BUNDLE_ARTIFACT" != "$BUNDLE_ARTIFACT" ]]; then
      rm -f "$CANONICAL_BUNDLE_ARTIFACT"
      cp "$BINARY_SRC" "$CANONICAL_BUNDLE_ARTIFACT"
      chmod +x "$CANONICAL_BUNDLE_ARTIFACT"
      echo "Created (compat): $CANONICAL_BUNDLE_ARTIFACT"
    fi
    ;;
  *)
    echo "Unsupported format: $FORMAT"
    exit 1
    ;;
esac

echo "Created: $BUNDLE_ARTIFACT"

