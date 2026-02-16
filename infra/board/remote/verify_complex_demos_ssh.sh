#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  cat <<'USAGE'
Usage:
  bash infra/board/remote/verify_complex_demos_ssh.sh <ssh_target> [remote_mount]

Examples:
  bash infra/board/remote/verify_complex_demos_ssh.sh root@192.168.0.106
  bash infra/board/remote/verify_complex_demos_ssh.sh root@192.168.0.106 /media/root/DAPLINK
USAGE
  exit 2
fi

SSH_TARGET="$1"
REMOTE_MOUNT="${2:-/media/root/DAPLINK}"
DEMO_DIR="/Users/jqwang/188-Gnucap/third_party/icesugar/demo"
OUT_DIR="infra/board/remote/evidence/complex_demo_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$OUT_DIR"

matrix=(
  "picorv32.bin:115200:20"
  "up5k_6502.bin:9600:20"
  "icicle.bin:9600:20"
  "litex-image-gateware+bios+none.bin:115200:25"
)

echo "== Complex Demo Verification (no LED dependency) ==" | tee "$OUT_DIR/summary.txt"
echo "Target: $SSH_TARGET" | tee -a "$OUT_DIR/summary.txt"
echo "Mount:  $REMOTE_MOUNT" | tee -a "$OUT_DIR/summary.txt"
echo "Time:   $(date '+%F %T %z')" | tee -a "$OUT_DIR/summary.txt"
echo | tee -a "$OUT_DIR/summary.txt"

for item in "${matrix[@]}"; do
  IFS=: read -r img baud secs <<<"$item"
  image_path="$DEMO_DIR/$img"
  if [[ ! -f "$image_path" ]]; then
    echo "[SKIP] missing image: $image_path" | tee -a "$OUT_DIR/summary.txt"
    continue
  fi

  echo "==== $img @ $baud ($secs s) ====" | tee -a "$OUT_DIR/summary.txt"

  bash infra/board/remote/flash_daplink_ssh.sh \
    "$image_path" \
    "$SSH_TARGET" \
    "$REMOTE_MOUNT" \
    "$img" \
    8 | tee "$OUT_DIR/${img}.flash.log"

  serial_bytes="$({
    ssh "$SSH_TARGET" "bash -s" -- "$baud" "$secs" <<'EOS'
set -euo pipefail
BAUD="$1"
SECS="$2"
DEV=/dev/ttyACM0
CAP="/tmp/complex_demo_cap_${BAUD}.bin"

stty -F "$DEV" "$BAUD" cs8 -cstopb -parenb -ixon -ixoff -echo -icanon min 0 time 1
rm -f "$CAP"
timeout "$SECS" dd if="$DEV" bs=1 count=4096 status=none > "$CAP" || true
BYTES=$(wc -c < "$CAP")
echo "BYTES=$BYTES"
if [[ "$BYTES" -gt 0 ]]; then
  echo "HEX_PREVIEW:"
  xxd -g 1 -u "$CAP" | sed -n '1,12p'
  echo "ASCII_PREVIEW:"
  tr -dc '\11\12\15\40-\176' < "$CAP" | head -c 240
  echo
fi
EOS
  } | tee "$OUT_DIR/${img}.serial.log" | awk -F= '/^BYTES=/{print $2}' | tail -1)"

  if [[ -z "$serial_bytes" ]]; then
    serial_bytes="0"
  fi

  echo "SERIAL_BYTES[$img]=$serial_bytes" | tee -a "$OUT_DIR/summary.txt"
  echo | tee -a "$OUT_DIR/summary.txt"
done

echo "Evidence dir: $OUT_DIR" | tee -a "$OUT_DIR/summary.txt"
