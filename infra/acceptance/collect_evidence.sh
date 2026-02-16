#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "$0")/../.." && pwd)
cd "$ROOT_DIR"

TS=$(date +"%Y%m%d_%H%M%S")
OUT_DIR="infra/acceptance/evidence/${TS}"

mkdir -p "$OUT_DIR"

echo "Collecting evidence into: $OUT_DIR"

{
  echo "timestamp: $(date -Iseconds)"
  echo "pwd: $(pwd)"
} > "${OUT_DIR}/meta.txt"

git rev-parse HEAD > "${OUT_DIR}/git_head.txt" 2>&1 || true
git status --short > "${OUT_DIR}/git_status.txt" 2>&1 || true

uname -a > "${OUT_DIR}/uname.txt" 2>&1 || true
sw_vers > "${OUT_DIR}/sw_vers.txt" 2>&1 || true

system_profiler SPHardwareDataType -detailLevel mini > "${OUT_DIR}/hardware.txt" 2>&1 || true
system_profiler SPUSBHostDataType -detailLevel full > "${OUT_DIR}/usb.txt" 2>&1 || true
system_profiler SPThunderboltDataType -detailLevel mini > "${OUT_DIR}/thunderbolt.txt" 2>&1 || true

ls -la /dev/cu.* /dev/tty.* > "${OUT_DIR}/dev_tty.txt" 2>&1 || true

if command -v openFPGALoader >/dev/null 2>&1; then
  openFPGALoader --detect > "${OUT_DIR}/openFPGALoader_detect.txt" 2>&1 || true
else
  echo "openFPGALoader not found in PATH" > "${OUT_DIR}/openFPGALoader_detect.txt"
fi

echo "Done."
echo "You can paste these paths into infra/acceptance/mode_acceptance.html → 证据归档："
echo "  $OUT_DIR"

