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
  echo "Format 'au' is only valid for target 'darwin'."
  exit 1
fi

echo "Building release plugin binary..."
TARGET_TRIPLE="$(target_triple_for "$TARGET")"
if [[ -n "$TARGET_TRIPLE" ]]; then
  cargo build --profile "$BUILD_PROFILE" --target "$TARGET_TRIPLE" --manifest-path "$ROOT_DIR/Cargo.toml"
else
  cargo build --profile "$BUILD_PROFILE" --manifest-path "$ROOT_DIR/Cargo.toml"
fi

BINARY_SRC="$(cargo_binary_path_for_target "$TARGET")"
if [[ ! -f "$BINARY_SRC" ]]; then
  echo "Expected built binary not found: $BINARY_SRC"
  echo "Set TARGET_TRIPLE_${TARGET^^} if cross-compiling to $TARGET."
  exit 1
fi

echo "Compile step complete for target: $TARGET"
