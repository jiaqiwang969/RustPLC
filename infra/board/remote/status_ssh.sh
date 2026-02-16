#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  cat <<'EOF'
Usage:
  bash infra/board/remote/status_ssh.sh <ssh_target> [tty_dev]

Examples:
  bash infra/board/remote/status_ssh.sh root@192.168.0.106
  bash infra/board/remote/status_ssh.sh wjq@192.168.0.106 /dev/ttyACM0
EOF
  exit 2
fi

SSH_TARGET="$1"
TTY_DEV="${2:-/dev/ttyACM0}"

echo "== Remote board status =="
echo "Target: $SSH_TARGET"
echo "Time:   $(date)"
echo

ssh "$SSH_TARGET" "bash -s" -- "$TTY_DEV" <<'EOS'
set -euo pipefail
TTY_DEV="$1"

echo "-- Host --"
uname -a
id
echo

echo "-- USB devices (lsusb) --"
if command -v lsusb >/dev/null 2>&1; then
  lsusb
else
  echo "lsusb not found"
fi
echo

echo "-- Serial devices (/dev/ttyACM* /dev/ttyUSB*) --"
found_serial=0
for pat in /dev/ttyACM* /dev/ttyUSB*; do
  if compgen -G "$pat" >/dev/null 2>&1; then
    ls -l $pat
    found_serial=1
  fi
done
if [[ "$found_serial" -eq 0 ]]; then
  echo "No ttyACM/ttyUSB device found"
fi
echo

if [[ -e "$TTY_DEV" ]]; then
  echo "-- Selected serial device: $TTY_DEV --"
  ls -l "$TTY_DEV"
  echo
  echo "Permission check:"
  if [[ -r "$TTY_DEV" && -w "$TTY_DEV" ]]; then
    echo "  OK: current user can read/write $TTY_DEV"
  else
    echo "  FAIL: current user cannot read/write $TTY_DEV"
  fi
  echo

  if command -v udevadm >/dev/null 2>&1; then
    echo "-- udev properties ($TTY_DEV) --"
    udevadm info -q property -n "$TTY_DEV" | grep -E 'ID_VENDOR|ID_MODEL|ID_SERIAL|ID_USB' || true
  fi
else
  echo "-- Selected serial device not present: $TTY_DEV --"
fi
echo

echo "-- dmesg tail (usb/serial) --"
dmesg | grep -i -E 'usb|serial|ftdi|ch34|cp210|cdc_acm|cmsis|dap' | tail -n 30 || true
EOS
