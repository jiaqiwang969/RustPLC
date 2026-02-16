# iCESugar UP5K Board Helpers

This folder contains minimal FPGA bring-up artifacts for iCESugar (iCE40UP5K).

## Green LED 10s toggle

Build:

```bash
bash infra/board/icesugar-up5k/blink_green_10s/build.sh
```

Simulation check:

```bash
bash infra/board/icesugar-up5k/blink_green_10s/run_sim.sh
```

Flash to a remote Linux host with DAPLink-mounted board:

```bash
bash infra/board/remote/flash_daplink_ssh.sh \
  infra/board/icesugar-up5k/blink_green_10s/build/blink_green_10s.bin \
  root@192.168.0.106
```

Behavior:
- Green LED toggles every 10 seconds.
- Full on+off cycle is 20 seconds.
