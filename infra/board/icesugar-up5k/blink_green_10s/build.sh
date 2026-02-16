#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
OUT_DIR="$SCRIPT_DIR/build"
OUT_BASE="blink_green_10s"
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
NEXTPNR_ICE40="$(resolve_tool NEXTPNR_ICE40 /Users/jqwang/oss-cad-suite/bin/nextpnr-ice40 nextpnr-ice40)"
ICEPACK="$(resolve_tool ICEPACK /Users/jqwang/oss-cad-suite/bin/icepack icepack)"

echo "Using tools:"
echo "  YOSYS=$YOSYS"
echo "  NEXTPNR_ICE40=$NEXTPNR_ICE40"
echo "  ICEPACK=$ICEPACK"

echo
"$YOSYS" -p "synth_ice40 -top top -json $OUT_DIR/$OUT_BASE.json" "$SCRIPT_DIR/top.v"
"$NEXTPNR_ICE40" \
  --up5k \
  --package sg48 \
  --json "$OUT_DIR/$OUT_BASE.json" \
  --pcf "$SCRIPT_DIR/top.pcf" \
  --asc "$OUT_DIR/$OUT_BASE.asc"
"$ICEPACK" "$OUT_DIR/$OUT_BASE.asc" "$OUT_DIR/$OUT_BASE.bin"

echo
ls -lh "$OUT_DIR/$OUT_BASE.bin"
echo "Build done: $OUT_DIR/$OUT_BASE.bin"
