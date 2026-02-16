#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
  cat <<'EOF'
Usage:
  bash infra/board/nucleo-f103-firmware/scripts/serial_sniff_ssh.sh <ssh_target> <tty_dev> [baud] [seconds]

Examples:
  bash infra/board/nucleo-f103-firmware/scripts/serial_sniff_ssh.sh root@192.168.0.106 /dev/ttyACM0
  bash infra/board/nucleo-f103-firmware/scripts/serial_sniff_ssh.sh root@192.168.0.106 /dev/ttyACM0 115200 5
EOF
  exit 2
fi

SSH_TARGET="$1"
TTY_DEV="$2"
BAUD="${3:-115200}"
SECONDS="${4:-5}"

echo "== Serial sniff (SSH) =="
echo "Target: $SSH_TARGET"
echo "Serial: $TTY_DEV"
echo "Baud:   $BAUD"
echo "Window: ${SECONDS}s"
echo

ssh "$SSH_TARGET" "bash -s" -- "$TTY_DEV" "$BAUD" "$SECONDS" <<'EOS'
set -euo pipefail
TTY_DEV="$1"
BAUD="$2"
SECONDS="$3"

stty -F "$TTY_DEV" "$BAUD" raw -echo
timeout "$SECONDS" cat "$TTY_DEV" || true
EOS
