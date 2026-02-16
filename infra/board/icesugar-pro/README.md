# iCESugar-pro Helpers

## 10s blink demo (ECP5 bitstream)

Build:

```bash
bash infra/board/icesugar-pro/blink_10s/build.sh
```

Simulation check:

```bash
bash infra/board/icesugar-pro/blink_10s/run_sim.sh
```

Flash to a remote DAPLink host:

```bash
bash infra/board/remote/flash_daplink_ssh.sh \
  infra/board/icesugar-pro/blink_10s/build/blink_10s.bit \
  root@192.168.0.106
```

Behavior:
- Green LED on pin `A12` toggles every 10s.
- Full on+off cycle is 20s.
