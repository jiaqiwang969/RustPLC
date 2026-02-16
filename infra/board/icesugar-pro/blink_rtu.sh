#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "$0")/../../.." && pwd)
cd "$ROOT_DIR"

PORT="${1:-}"
DEVICE="${2:-valve_extend}"
PERIOD_MS="${3:-200}"
COUNT="${4:-6}"

if [[ -z "$PORT" ]]; then
  # Try best-effort auto-detect on macOS.
  # Common USB-UART names on macOS:
  # - FTDI:        /dev/cu.usbserial-*
  # - CDC ACM:     /dev/cu.usbmodem*
  # - CP210x:      /dev/cu.SLAB_USBtoUART*
  # - CH34x/WCH:   /dev/cu.wchusbserial*
  PORT=$(
    ls /dev/cu.usbserial* /dev/cu.usbmodem* /dev/cu.SLAB_USBtoUART* /dev/cu.wchusbserial* 2>/dev/null \
      | head -n 1 \
      || true
  )
fi

if [[ -z "$PORT" ]]; then
  echo "No USB serial port detected."
  echo "Run: bash infra/board/icesugar-pro/status.sh"
  echo "Common causes: charge-only USB-C cable / hub / board not powered."
  exit 1
fi

TMP_CFG=$(mktemp /tmp/rustplc_icesugar_rtu_XXXX.toml)
trap 'rm -f \"$TMP_CFG\"' EXIT

sed "s|/dev/cu.usbserial-REPLACE_ME|$PORT|g" config/hal_icesugar_pro_rtu.toml > "$TMP_CFG"

echo "Using serial port: $PORT"
echo "Using temp config: $TMP_CFG"
echo "Blink: device=$DEVICE period_ms=$PERIOD_MS count=$COUNT"

cargo run -p rustplc_orchestrator --bin blink -- "$TMP_CFG" "$DEVICE" "$PERIOD_MS" "$COUNT"
