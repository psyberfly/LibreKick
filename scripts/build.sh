#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/source_env.sh"

TARGETS_INPUT="${1:-${TARGETS_RAW:-${TARGET:-}}}"
FORMATS_INPUT="${2:-${FORMATS_RAW:-${FORMAT:-vst3}}}"

if [[ -z "$TARGETS_INPUT" ]]; then
  echo "target is not set. Define TARGETS/TARGET in scripts/config.env (e.g. TARGETS=linux,darwin)."
  exit 1
fi

parse_csv_list "$TARGETS_INPUT" REQUESTED_TARGETS
parse_csv_list "$FORMATS_INPUT" REQUESTED_FORMATS_RAW

if [[ ${#REQUESTED_TARGETS[@]} -eq 0 ]]; then
  echo "No valid targets provided."
  exit 1
fi

if [[ ${#REQUESTED_FORMATS_RAW[@]} -eq 0 ]]; then
  echo "No valid formats provided."
  exit 1
fi

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

for target in "${REQUESTED_TARGETS[@]}"; do
  if ! is_supported_target "$target"; then
    echo "Unsupported target: $target"
    echo "Supported targets: linux, darwin, windows"
    exit 1
  fi

  "$SCRIPT_DIR/core/compile.sh" "$target"

  for format in "${REQUESTED_FORMATS[@]}"; do
    if [[ "$format" == "au" && "$target" != "darwin" ]]; then
      echo "Skipping format 'au' for target '$target' (au is darwin-only)."
      continue
    fi

    "$SCRIPT_DIR/core/bundle.sh" "$target" "$format"
  done
done

echo "Build pipeline complete for targets: ${REQUESTED_TARGETS[*]}"
