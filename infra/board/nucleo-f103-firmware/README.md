# NUCLEO-F103RB Firmware (DAPLink)

This folder provides two minimal firmware binaries for a DAPLink/NUCLEO-F103RB style board:

- `blink`: toggles LED (`PA5` and `PC13`) and writes text to UART2 (`115200`).
- `modbus_slave`: Modbus RTU slave on UART2 (`115200`, unit id `1`), with:
  - `FC01` Read Coils
  - `FC02` Read Discrete Inputs
  - `FC05` Write Single Coil
  - `FC0F` Write Multiple Coils

## Build

```bash
bash infra/board/nucleo-f103-firmware/scripts/build.sh blink
bash infra/board/nucleo-f103-firmware/scripts/build.sh modbus_slave
```

## Flash over SSH (DAPLINK mass storage)

```bash
bash infra/board/nucleo-f103-firmware/scripts/flash_ssh.sh \
  infra/board/nucleo-f103-firmware/target/thumbv7m-none-eabi/release/blink.bin \
  root@192.168.0.106 \
  /media/wjq/DAPLINK
```

For Modbus slave firmware, replace `blink.bin` with `modbus_slave.bin`.

## Verify blink serial output

```bash
bash infra/board/nucleo-f103-firmware/scripts/serial_sniff_ssh.sh \
  root@192.168.0.106 \
  /dev/ttyACM0 \
  115200 \
  5
```

## Verify Modbus RTU response

```bash
bash infra/board/remote/modbus_probe_ssh.sh root@192.168.0.106 /dev/ttyACM0 1
```

If probe still shows no response, check:
- board target wiring / jumpers for UART2 path;
- correct UART mapped to DAPLink VCP;
- board clock/boot mode/firmware actually flashed.

## Common failure

If `FAIL.TXT` says:

`The interface firmware FAILED to reset/halt the target MCU`

then DAPLink can enumerate over USB, but cannot control the target MCU over SWD (target disconnected, unpowered, or held/reset/locked).
