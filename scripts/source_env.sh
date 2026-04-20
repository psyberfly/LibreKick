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

if [[ -z "${TARGET:-}" && -n "${OS:-}" ]]; then
  TARGET="$OS"
fi

trim_string() {
  local value="$1"
  value="${value#"${value%%[![:space:]]*}"}"
  value="${value%"${value##*[![:space:]]}"}"
  printf '%s' "$value"
}

canonical_bundle_artifact_path_for() {
  local target="${1,,}"
  local format=""
  local root=""
  local canonical_basename=""
  format="$(normalize_format "$2")"
  root="$(format_output_root_for "$format")"
  canonical_basename="$(install_bundle_basename_for "$format")"

  if [[ "$format" == "au" && "$target" != "darwin" ]]; then
    echo ""
  else
    echo "$root/$canonical_basename"
  fi
}

parse_csv_list() {
  local raw="$1"
  local -n out_ref="$2"
  local -a parts=()
  local item=""

  out_ref=()
  IFS=',' read -r -a parts <<< "$raw"
  for item in "${parts[@]}"; do
    item="$(trim_string "$item")"
    if [[ -n "$item" ]]; then
      out_ref+=("${item,,}")
    fi
  done
}

is_supported_target() {
  local target="${1,,}"
  case "$target" in
    linux|darwin|windows) return 0 ;;
    *) return 1 ;;
  esac
}

canonical_bundle_binary_path_for() {
  local target="${1,,}"
  local format=""
  local artifact=""
  local ext=""
  format="$(normalize_format "$2")"
  artifact="$(canonical_bundle_artifact_path_for "$target" "$format")"

  case "$format" in
    vst3)
      ext="$(binary_extension_for_target "$target")"
      echo "$artifact/Contents/$(target_arch_dir_for "$target")/${PLUGIN_NAME}${ext}"
      ;;
    clap3)
      echo "$artifact"
      ;;
    au)
      if [[ "$target" != "darwin" ]]; then
        echo ""
      else
        echo "$artifact/Contents/MacOS/${PLUGIN_NAME}"
      fi
      ;;
    *) return 1 ;;
  esac
}

release_bundle_basename_for() {
  local target="${1,,}"
  local format=""
  local arch=""
  format="$(normalize_format "$2")"
  arch="$(target_arch_for "$target")"

  case "$format" in
    vst3) echo "${PLUGIN_NAME}_${target}_${arch}.vst3" ;;
    clap3) echo "${PLUGIN_NAME}_${target}_${arch}.clap" ;;
    au)
      if [[ "$target" != "darwin" ]]; then
        echo ""
      else
        echo "${PLUGIN_NAME}_${target}_${arch}.component"
      fi
      ;;
    *) return 1 ;;
  esac
}

install_bundle_basename_for() {
  local format=""
  format="$(normalize_format "$1")"

  case "$format" in
    vst3) echo "${PLUGIN_NAME}.vst3" ;;
    clap3) echo "${PLUGIN_NAME}.clap" ;;
    au) echo "${PLUGIN_NAME}.component" ;;
    *) return 1 ;;
  esac
}

normalize_format() {
  local format="${1,,}"
  case "$format" in
    vst3) echo "vst3" ;;
    clap|clap3) echo "clap3" ;;
    au) echo "au" ;;
    *) return 1 ;;
  esac
}

is_supported_format() {
  normalize_format "$1" >/dev/null 2>&1
}

target_triple_for() {
  local target="${1,,}"
  case "$target" in
    linux) echo "${TARGET_TRIPLE_LINUX:-}" ;;
    darwin) echo "${TARGET_TRIPLE_DARWIN:-}" ;;
    windows) echo "${TARGET_TRIPLE_WINDOWS:-}" ;;
    *) echo "" ;;
  esac
}

target_arch_dir_for() {
  local target="${1,,}"
  case "$target" in
    linux) echo "x86_64-linux" ;;
    darwin) echo "x86_64-macos" ;;
    windows) echo "x86_64-win" ;;
    *) return 1 ;;
  esac
}

target_arch_for() {
  local target="${1,,}"
  case "$target" in
    linux|darwin|windows) echo "x86_64" ;;
    *) return 1 ;;
  esac
}

binary_extension_for_target() {
  local target="${1,,}"
  case "$target" in
    linux) echo ".so" ;;
    darwin) echo ".dylib" ;;
    windows) echo ".dll" ;;
    *) return 1 ;;
  esac
}

PLUGIN_NAME="LibreKick"
BUILD_PROFILE="${BUILD_PROFILE:-release}"

TARGETS_RAW="${TARGETS:-${TARGET:-}}"
if [[ -z "$TARGETS_RAW" ]]; then
  TARGETS_RAW="${OS:-}"
fi

if [[ -z "$TARGETS_RAW" ]]; then
  echo "TARGETS/TARGET must be set in config.env (e.g. TARGETS=linux,darwin)"
  exit 1
fi

FORMATS_RAW="${FORMATS:-${FORMAT:-clap3}}"

parse_csv_list "$TARGETS_RAW" TARGET_LIST
parse_csv_list "$FORMATS_RAW" FORMAT_LIST_RAW

if [[ ${#TARGET_LIST[@]} -eq 0 ]]; then
  echo "No valid targets found in TARGETS/TARGET"
  exit 1
fi

if [[ ${#FORMAT_LIST_RAW[@]} -eq 0 ]]; then
  echo "No valid formats found in FORMATS/FORMAT"
  exit 1
fi

FORMAT_LIST=()
for _format in "${FORMAT_LIST_RAW[@]}"; do
  if normalized="$(normalize_format "$_format")"; then
    FORMAT_LIST+=("$normalized")
  else
    echo "Unsupported format in config: $_format"
    echo "Supported formats: vst3, clap3, au"
    exit 1
  fi
done

for _target in "${TARGET_LIST[@]}"; do
  if ! is_supported_target "$_target"; then
    echo "Unsupported target in config: $_target"
    echo "Supported targets: linux, darwin, windows"
    exit 1
  fi
done

TARGET="${TARGET_LIST[0]}"
FORMAT="${FORMAT_LIST[0]}"

PLUGIN_BINARY_BASENAME="liblibrekick"

binary_filename_for_target() {
  local target="${1,,}"
  local ext=""
  ext="$(binary_extension_for_target "$target")"
  echo "${PLUGIN_BINARY_BASENAME}${ext}"
}

cargo_binary_path_for_target() {
  local target="${1,,}"
  local target_triple=""
  local output_dir=""

  target_triple="$(target_triple_for "$target")"
  if [[ -n "$target_triple" ]]; then
    output_dir="$ROOT_DIR/target/$target_triple/$BUILD_PROFILE"
  else
    output_dir="$ROOT_DIR/target/$BUILD_PROFILE"
  fi

  echo "$output_dir/$(binary_filename_for_target "$target")"
}

format_output_root_for() {
  local format=""
  local build_base=""
  format="$(normalize_format "$1")"
  build_base="${BUILD_PATH:-target}"
  build_base="${build_base%/}"

  case "$format" in
    vst3) echo "$ROOT_DIR/${build_base}/vst3" ;;
    clap3) echo "$ROOT_DIR/${build_base}/clap" ;;
    au) echo "$ROOT_DIR/${build_base}/au" ;;
    *) return 1 ;;
  esac
}

bundle_artifact_path_for() {
  local target="${1,,}"
  local format=""
  local root=""
  local artifact_basename=""
  format="$(normalize_format "$2")"
  root="$(format_output_root_for "$format")"
  artifact_basename="$(release_bundle_basename_for "$target" "$format")"

  if [[ -z "$artifact_basename" ]]; then
    echo ""
  else
    echo "$root/$artifact_basename"
  fi
}

bundle_binary_path_for() {
  local target="${1,,}"
  local format=""
  local artifact=""
  local ext=""
  format="$(normalize_format "$2")"
  artifact="$(bundle_artifact_path_for "$target" "$format")"

  case "$format" in
    vst3)
      ext="$(binary_extension_for_target "$target")"
      echo "$artifact/Contents/$(target_arch_dir_for "$target")/${PLUGIN_NAME}${ext}"
      ;;
    clap3)
      echo "$artifact"
      ;;
    au)
      if [[ "$target" != "darwin" ]]; then
        echo ""
      else
        echo "$artifact/Contents/MacOS/${PLUGIN_NAME}"
      fi
      ;;
    *) return 1 ;;
  esac
}

install_root_for() {
  local target="${1,,}"
  local format=""
  format="$(normalize_format "$2")"

  case "$target:$format" in
    linux:vst3) echo "${VST3_INSTALL_PATH_LINUX:-${VST_INSTALL_PATH:-$HOME/.vst3}}" ;;
    linux:clap3) echo "${CLAP_INSTALL_PATH_LINUX:-$HOME/.clap}" ;;
    linux:au) echo "" ;;
    darwin:vst3) echo "${VST3_INSTALL_PATH_DARWIN:-$HOME/Library/Audio/Plug-Ins/VST3}" ;;
    darwin:clap3) echo "${CLAP_INSTALL_PATH_DARWIN:-$HOME/Library/Audio/Plug-Ins/CLAP}" ;;
    darwin:au) echo "${AU_INSTALL_PATH_DARWIN:-$HOME/Library/Audio/Plug-Ins/Components}" ;;
    windows:vst3) echo "${VST3_INSTALL_PATH_WINDOWS:-$HOME/AppData/Local/Common Files/VST3}" ;;
    windows:clap3) echo "${CLAP_INSTALL_PATH_WINDOWS:-$HOME/.clap}" ;;
    windows:au) echo "" ;;
    *) echo "" ;;
  esac
}

LOCAL_VST3_DIR="$(format_output_root_for vst3)"
PLUGIN_BUNDLE_DIR="$(bundle_artifact_path_for "$TARGET" vst3)"
BINARY_SRC="$(cargo_binary_path_for_target "$TARGET")"
BINARY_DST="$(bundle_binary_path_for "$TARGET" vst3)"
BINARY_DST_DIR="$(dirname "$BINARY_DST")"
INSTALL_BUNDLE_DIR="$(install_root_for "$TARGET" vst3)/${PLUGIN_NAME}.vst3"
