# iCESugar UP5K Modbus RTU Slave (115200)

This demo implements a minimal Modbus RTU slave in FPGA fabric.

- UART: `115200 8N1`
- Function supported: `FC01` (read coils)
- Request accepted: `start=0, qty=1`
- Response coil value: fixed `1`

## Build

```bash
bash infra/board/icesugar-up5k/modbus_rtu_slave_115200/build.sh
```

## Flash + probe

```bash
bash infra/board/remote/flash_daplink_ssh.sh \
  infra/board/icesugar-up5k/modbus_rtu_slave_115200/build/modbus_rtu_slave_115200.bin \
  root@192.168.0.106 \
  /media/root/DAPLINK \
  modbus_rtu_slave_115200.bin \
  8

bash infra/board/remote/modbus_probe_ssh.sh root@192.168.0.106 /dev/ttyACM0 1
```

## Pin map

- `RX` -> FPGA pin `4`
- `TX` -> FPGA pin `6`

See `../modbus_rtu_slave_115200_swap` for swapped TX/RX mapping test.
