#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/source_env.sh"

TARGETS_INPUT="${1:-${TARGETS_RAW:-${TARGET:-}}}"
FORMATS_INPUT="${2:-${FORMATS_RAW:-${FORMAT:-clap3}}}"

if [[ -z "$TARGETS_INPUT" ]]; then
  echo "target is not set. Define TARGETS/TARGET in scripts/config.env (e.g. TARGETS=linux,darwin)."
  exit 1
fi

parse_csv_list "$TARGETS_INPUT" REQUESTED_TARGETS
parse_csv_list "$FORMATS_INPUT" REQUESTED_FORMATS_RAW

REQUESTED_FORMATS=()
for format in "${REQUESTED_FORMATS_RAW[@]}"; do
  if normalized_format="$(normalize_format "$format")"; then
    REQUESTED_FORMATS+=("$normalized_format")
  else
    echo "Unsupported format: $format"
    echo "Supported formats: vst3, clap3, au"
    exit 1
  fi
done

echo "Running build pipeline before install..."
"$SCRIPT_DIR/build.sh" "$TARGETS_INPUT" "$FORMATS_INPUT"

for target in "${REQUESTED_TARGETS[@]}"; do
  if ! is_supported_target "$target"; then
    echo "Unsupported target: $target"
    echo "Supported targets: linux, darwin, windows"
    exit 1
  fi

  for format in "${REQUESTED_FORMATS[@]}"; do
    if [[ "$format" == "au" && "$target" != "darwin" ]]; then
      echo "Skipping install for format 'au' on target '$target' (au is darwin-only)."
      continue
    fi

    "$SCRIPT_DIR/core/install.sh" "$target" "$format"
  done
done
