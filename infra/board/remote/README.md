# Remote Board Helpers (SSH)

When a development board is connected to another Linux host (for example `root@192.168.0.106`), use these scripts to keep hardware checks repeatable.

## 1) Enumerate board + serial status

```bash
bash infra/board/remote/status_ssh.sh root@192.168.0.106
```

This collects:
- host/arch info (`uname -a`, `id`)
- `lsusb`
- `/dev/ttyACM*` and `/dev/ttyUSB*`
- selected serial device permission check
- recent `dmesg` USB/serial lines

## 2) Probe Modbus RTU response

```bash
bash infra/board/remote/modbus_probe_ssh.sh root@192.168.0.106 /dev/ttyACM0 1
```

It sends a standard Modbus RTU request (`FC01 read coil[0]`) at common baud rates:
- 9600, 19200, 38400, 57600, 115200

If all reads are empty, the board is visible to OS but does not currently respond as a Modbus RTU slave.

## 3) Drag-drop flash to DAPLink/iCELink

```bash
bash infra/board/remote/flash_daplink_ssh.sh \
  /Users/jqwang/188-Gnucap/third_party/icesugar-pro/demo/blink_red.bit \
  root@192.168.0.106
```

What it does:
- copies local image to remote `/tmp`
- auto-detects mount path (`/media/*/DAPLINK`, `/run/media/*/DAPLINK`, and iCELink variants)
- removes stale `FAIL.TXT` before flashing
- drag-drops the image and checks for new `FAIL.TXT`

Notes:
- This is a best-effort signal (`FAIL.TXT` absent != guaranteed functional logic).
- For physical verification (LED blink, UART output), pair this with on-site observation.

## 4) One-shot board connectivity test (recommended)

```bash
bash infra/board/remote/board_connectivity_test_ssh.sh \
  root@192.168.0.106 \
  /media/root/DAPLINK \
  /dev/ttyACM0 \
  /Users/jqwang/188-Gnucap/third_party/icesugar/demo/leds.bin \
  /Users/jqwang/199-rustplc/RustPLC/infra/board/icesugar-up5k/blink_green_10s/build/blink_green_10s.bin
```

This verifies:
- USB DAPLink enumeration (`0d28:0204`)
- serial device existence (`/dev/ttyACM0`)
- DAPLink mount path
- programming cycle #1 + `FAIL.TXT` check
- programming cycle #2 + `FAIL.TXT` check

If all items are `PASS`, host-to-board connection and drag-drop flashing path are healthy.

## 5) Verify complex demos without LED observation

```bash
bash infra/board/remote/verify_complex_demos_ssh.sh \
  root@192.168.0.106 \
  /media/root/DAPLINK
```

This test matrix flashes multiple advanced UP5K demos and captures serial output:
- `picorv32.bin` @ 115200
- `up5k_6502.bin` @ 9600
- `icicle.bin` @ 9600
- `litex-image-gateware+bios+none.bin` @ 115200

Logs are saved under `infra/board/remote/evidence/complex_demo_YYYYmmdd_HHMMSS/`.

## 6) Probe all serial ports for Modbus RTU

```bash
bash infra/board/remote/modbus_probe_all_serial_ssh.sh root@192.168.0.106 1
```

This auto-detects `/dev/ttyUSB*` and `/dev/ttyACM*` on the remote host and runs
`modbus_probe_ssh.sh` on each port. Useful after plugging an external USB-UART
adapter for FPGA direct serial validation.
