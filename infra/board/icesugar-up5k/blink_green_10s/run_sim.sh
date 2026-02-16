#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
OUT_DIR="$SCRIPT_DIR/build"
mkdir -p "$OUT_DIR"

IVERILOG="${IVERILOG:-$(command -v iverilog)}"
VVP="${VVP:-$(command -v vvp)}"

if [[ -z "$IVERILOG" || -z "$VVP" ]]; then
  echo "ERROR: iverilog/vvp not found" >&2
  exit 1
fi

"$IVERILOG" -g2012 -o "$OUT_DIR/tb_top.out" \
  "$SCRIPT_DIR/tb_top.v" \
  "$SCRIPT_DIR/top.v"

"$VVP" "$OUT_DIR/tb_top.out"
