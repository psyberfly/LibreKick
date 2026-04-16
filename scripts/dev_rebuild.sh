#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/source_env.sh"

echo "[1/3] compile_linux.sh"
bash "$ROOT_DIR/scripts/compile_linux.sh"

echo "[2/3] bundle_linux_vst3.sh"
bash "$ROOT_DIR/scripts/bundle_linux_vst3.sh"

echo "[3/3] install_linux.sh"
bash "$ROOT_DIR/scripts/install_linux.sh"

echo "Done: dev rebuild pipeline completed."
