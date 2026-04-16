#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/source_env.sh"

LATEST_PLUGIN_SO="$BINARY_DST"

needs_rebuild() {
  if [[ ! -f "$LATEST_PLUGIN_SO" ]]; then
    return 0
  fi

  if find "$ROOT_DIR/src" "$ROOT_DIR/scripts" "$ROOT_DIR/Cargo.toml" \
    -type f -newer "$LATEST_PLUGIN_SO" -print -quit | grep -q .; then
    return 0
  fi

  return 1
}

restart_carla() {
  local stopped=0

  for proc in carla-single carla carla2; do
    if pgrep -x "$proc" >/dev/null 2>&1; then
      pkill -x "$proc" || true
      stopped=1
    fi
  done

  if [[ "$stopped" -eq 1 ]]; then
    sleep 1
  fi
}

if needs_rebuild; then
  echo "[1/2] Build is missing or outdated. Rebuilding..."
  bash "$ROOT_DIR/scripts/compile_linux.sh"
else
  echo "[1/2] Build is up to date. Skipping rebuild."
fi

echo "[2/2] Launching latest build in Carla..."
restart_carla
exec bash "$ROOT_DIR/scripts/start_linux.sh"
