#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/source_env.sh"

echo "Building release plugin binary..."
cargo build --profile "$BUILD_PROFILE" --manifest-path "$ROOT_DIR/Cargo.toml"

echo "Bundling Linux VST3..."
"$ROOT_DIR/scripts/bundle_linux_vst3.sh"

echo "Done. Linux VST3 bundle is in: $LOCAL_VST3_DIR"
