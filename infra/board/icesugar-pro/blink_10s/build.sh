#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
OUT_DIR="$SCRIPT_DIR/build"
OUT_BASE="blink_10s"
mkdir -p "$OUT_DIR"

resolve_tool() {
  local env_name="$1"
  local default_bin="$2"
  local fallback_name="$3"

  local candidate="${!env_name:-}"
  if [[ -n "$candidate" ]]; then
    echo "$candidate"
    return 0
  fi

  if [[ -x "$default_bin" ]]; then
    echo "$default_bin"
    return 0
  fi

  if command -v "$fallback_name" >/dev/null 2>&1; then
    command -v "$fallback_name"
    return 0
  fi

  echo "ERROR: tool not found: $fallback_name" >&2
  exit 1
}

YOSYS="$(resolve_tool YOSYS /Users/jqwang/oss-cad-suite/bin/yosys yosys)"
NEXTPNR_ECP5="$(resolve_tool NEXTPNR_ECP5 /Users/jqwang/oss-cad-suite/bin/nextpnr-ecp5 nextpnr-ecp5)"
ECPPACK="$(resolve_tool ECPPACK /Users/jqwang/oss-cad-suite/bin/ecppack ecppack)"

echo "Using tools:"
echo "  YOSYS=$YOSYS"
echo "  NEXTPNR_ECP5=$NEXTPNR_ECP5"
echo "  ECPPACK=$ECPPACK"

echo
"$YOSYS" -p "synth_ecp5 -top $OUT_BASE -json $OUT_DIR/$OUT_BASE.json" \
  "$SCRIPT_DIR/$OUT_BASE.v" "$SCRIPT_DIR/rst_gen.v"
"$NEXTPNR_ECP5" \
  --25k \
  --package CABGA256 \
  --speed 6 \
  --json "$OUT_DIR/$OUT_BASE.json" \
  --textcfg "$OUT_DIR/$OUT_BASE.config" \
  --lpf "$SCRIPT_DIR/$OUT_BASE.lpf" \
  --freq 25
"$ECPPACK" --svf "$OUT_DIR/$OUT_BASE.svf" "$OUT_DIR/$OUT_BASE.config" "$OUT_DIR/$OUT_BASE.bit"

echo
ls -lh "$OUT_DIR/$OUT_BASE.bit"
echo "Build done: $OUT_DIR/$OUT_BASE.bit"
