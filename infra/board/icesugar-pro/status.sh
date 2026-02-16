#!/usr/bin/env bash
set -euo pipefail

echo "== iCESugar-pro v1.3 status (macOS) =="
echo "Time: $(date)"
echo

echo "-- USB (system_profiler SPUSBHostDataType -detailLevel mini) --"
system_profiler SPUSBHostDataType -detailLevel mini 2>/dev/null || true
echo

echo "-- Serial ports (/dev/cu.*) --"
ls -la /dev/cu.* 2>/dev/null || true
echo

echo "-- openFPGALoader --detect --"
if command -v openFPGALoader >/dev/null 2>&1; then
  openFPGALoader --detect 2>&1 || true
else
  echo "openFPGALoader not found in PATH"
fi
echo

cat <<'EOF'
Notes:
- If you see no USB device and openFPGALoader says "device not found", the most common原因是:
  1) USB-C 线是"充电线"(无数据线)；换一根支持数据的线；
  2) USB hub/转接器问题；直连电脑端口；
  3) 板子未上电/接触不良；重插并观察板载电源/状态灯。

- 如果板子枚举出了串口，一般会出现类似：
  /dev/cu.usbserial-xxxx  或  /dev/cu.usbmodemxxxx
  后续可用 RustPLC 的 modbus_rtu 模式连接（见 config/hal_modbus.toml）。
EOF

