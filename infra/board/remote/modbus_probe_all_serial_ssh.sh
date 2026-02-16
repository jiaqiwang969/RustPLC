#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  cat <<'USAGE'
Usage:
  bash infra/board/remote/modbus_probe_all_serial_ssh.sh <ssh_target> [unit_id]

Examples:
  bash infra/board/remote/modbus_probe_all_serial_ssh.sh root@192.168.0.106
  bash infra/board/remote/modbus_probe_all_serial_ssh.sh root@192.168.0.106 1
USAGE
  exit 2
fi

SSH_TARGET="$1"
UNIT_ID="${2:-1}"

echo "== Modbus RTU probe on all serial devices =="
echo "Target:  $SSH_TARGET"
echo "Unit ID: $UNIT_ID"
echo

serials_raw="$(
  ssh "$SSH_TARGET" 'bash -lc '\''for d in /dev/ttyUSB* /dev/ttyACM*; do [ -c "$d" ] && echo "$d"; done'\''' 2>/dev/null | sort -u
)"

serials=()
while IFS= read -r dev; do
  [[ -n "$dev" ]] && serials+=("$dev")
done <<< "$serials_raw"

if [[ "${#serials[@]}" -eq 0 ]]; then
  echo "No /dev/ttyUSB* or /dev/ttyACM* found on target."
  exit 1
fi

for dev in "${serials[@]}"; do
  echo "===== Probe $dev ====="
  bash infra/board/remote/modbus_probe_ssh.sh "$SSH_TARGET" "$dev" "$UNIT_ID"
  echo
done
