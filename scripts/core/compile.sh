#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../source_env.sh"

TARGET="${1:-${TARGET:-}}"

if [[ -z "$TARGET" ]]; then
  echo "target is not set. Define TARGET in scripts/config.env (e.g. TARGET=linux)."
  exit 1
fi

echo "Building release plugin binary..."
cargo build --profile "$BUILD_PROFILE" --manifest-path "$ROOT_DIR/Cargo.toml"

case "$TARGET" in
  linux)
    echo "Compile step complete for target: $TARGET"
    ;;
  *)
    echo "Unsupported target: $TARGET"
    echo "Currently supported: linux"
    exit 1
    ;;
esac
