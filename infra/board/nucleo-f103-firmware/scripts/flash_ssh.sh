#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 3 ]]; then
  cat <<'EOF'
Usage:
  bash infra/board/nucleo-f103-firmware/scripts/flash_ssh.sh <bin_path> <ssh_target> <daplink_mount>

Examples:
  bash infra/board/nucleo-f103-firmware/scripts/flash_ssh.sh \
    infra/board/nucleo-f103-firmware/target/thumbv7m-none-eabi/release/blink.bin \
    root@192.168.0.106 \
    /media/wjq/DAPLINK
EOF
  exit 2
fi

BIN_PATH="$1"
SSH_TARGET="$2"
DAPLINK_MOUNT="$3"

if [[ ! -f "$BIN_PATH" ]]; then
  echo "Firmware binary not found: $BIN_PATH"
  exit 1
fi

REMOTE_TMP="/tmp/$(basename "$BIN_PATH")"

echo "== Flash via DAPLINK (SSH) =="
echo "Binary: $BIN_PATH"
echo "Target: $SSH_TARGET"
echo "Mount:  $DAPLINK_MOUNT"
echo

scp "$BIN_PATH" "$SSH_TARGET:$REMOTE_TMP"

ssh "$SSH_TARGET" "bash -s" -- "$REMOTE_TMP" "$DAPLINK_MOUNT" <<'EOS'
set -euo pipefail
BIN="$1"
MNT="$2"

if [[ ! -d "$MNT" ]]; then
  echo "DAPLINK mount not found: $MNT"
  exit 1
fi

echo "Copying firmware to $MNT/firmware.bin"
cp "$BIN" "$MNT/firmware.bin"
sync
sleep 1

# DAPLink often disconnects and remounts after a drag-drop flash.
for _ in $(seq 1 20); do
  if [[ -d "$MNT" ]]; then
    break
  fi
  sleep 0.5
done

echo
echo "Post-flash check:"
if [[ ! -d "$MNT" ]]; then
  echo "DAPLINK mount disappeared and did not remount in time: $MNT"
  exit 1
fi

if [[ -f "$MNT/FAIL.TXT" ]]; then
  echo "FAIL.TXT found:"
  for _ in $(seq 1 10); do
    if sed -n '1,120p' "$MNT/FAIL.TXT"; then
      break
    fi
    sleep 0.2
  done
  exit 1
fi

echo "No FAIL.TXT found (best-effort success signal)."
ls -la "$MNT" | sed -n '1,40p'
EOS
