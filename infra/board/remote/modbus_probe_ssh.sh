#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  cat <<'EOF'
Usage:
  bash infra/board/remote/modbus_probe_ssh.sh <ssh_target> [tty_dev] [unit_id]

Examples:
  bash infra/board/remote/modbus_probe_ssh.sh root@192.168.0.106
  bash infra/board/remote/modbus_probe_ssh.sh root@192.168.0.106 /dev/ttyACM0 1
EOF
  exit 2
fi

SSH_TARGET="$1"
TTY_DEV="${2:-/dev/ttyACM0}"
UNIT_ID="${3:-1}"

echo "== Remote Modbus RTU probe =="
echo "Target:   $SSH_TARGET"
echo "Serial:   $TTY_DEV"
echo "Unit ID:  $UNIT_ID"
echo "Request:  FC01 read coil[0], qty=1"
echo

cat <<'PY' | ssh "$SSH_TARGET" "python3 -" "$TTY_DEV" "$UNIT_ID"
import os
import select
import sys
import termios
import time

args = [a for a in sys.argv[1:] if a != "--"]
if len(args) < 2:
    print("usage: python3 probe.py <tty_dev> <unit_id>", file=sys.stderr)
    raise SystemExit(2)

tty_dev = args[0]
unit_id = int(args[1]) & 0xFF

if not os.path.exists(tty_dev):
    print(f"ERROR: serial device not found: {tty_dev}")
    raise SystemExit(1)

def crc16_modbus(data: bytes) -> int:
    crc = 0xFFFF
    for b in data:
        crc ^= b
        for _ in range(8):
            if crc & 1:
                crc = (crc >> 1) ^ 0xA001
            else:
                crc >>= 1
    return crc & 0xFFFF

def build_read_coils_req(uid: int, start: int, qty: int) -> bytes:
    core = bytes([
        uid,
        0x01,
        (start >> 8) & 0xFF,
        start & 0xFF,
        (qty >> 8) & 0xFF,
        qty & 0xFF,
    ])
    crc = crc16_modbus(core)
    return core + bytes([crc & 0xFF, (crc >> 8) & 0xFF])

req = build_read_coils_req(unit_id, 0, 1)

baud_table = [
    (9600, termios.B9600),
    (19200, termios.B19200),
    (38400, termios.B38400),
    (57600, termios.B57600),
    (115200, termios.B115200),
]

got_any_reply = False
for baud, speed in baud_table:
    fd = os.open(tty_dev, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
    try:
        attrs = termios.tcgetattr(fd)
        attrs[0] = 0
        attrs[1] = 0
        attrs[2] = termios.CS8 | termios.CREAD | termios.CLOCAL
        attrs[3] = 0
        attrs[4] = speed
        attrs[5] = speed
        termios.tcsetattr(fd, termios.TCSANOW, attrs)
        termios.tcflush(fd, termios.TCIOFLUSH)

        os.write(fd, req)
        end = time.time() + 0.35
        buf = b""
        while time.time() < end:
            r, _, _ = select.select([fd], [], [], 0.05)
            if fd in r:
                try:
                    chunk = os.read(fd, 256)
                except BlockingIOError:
                    chunk = b""
                if chunk:
                    buf += chunk

        if buf:
            got_any_reply = True
        print(f"baud={baud:<6} tx={req.hex()} rx={buf.hex()} len={len(buf)}")
    finally:
        os.close(fd)

print()
if got_any_reply:
    print("RESULT: got serial response. The device may be alive as Modbus RTU (or at least talking).")
else:
    print("RESULT: no response at common baud rates.")
    print("Hint: check target firmware / unit_id / wiring / power / RS-485 transceiver.")
PY
