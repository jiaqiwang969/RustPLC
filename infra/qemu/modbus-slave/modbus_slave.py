#!/usr/bin/env python3
"""Modbus TCP slave simulator for RustPLC Mode B testing.

Runs inside a QEMU Ubuntu VM. Exposes coils and discrete inputs over
Modbus TCP port 502. Simulates simple cylinder physics:
  - Coils (FC 0x01/0x05/0x0F): writable outputs (valves)
  - Discrete Inputs (FC 0x02): readable inputs (sensors)

Physics model:
  - valve_extend ON for 3+ cycles → sensor_end = HIGH, sensor_home = LOW
  - valve_retract ON for 3+ cycles → sensor_home = HIGH, sensor_end = LOW

Dependencies: pymodbus >= 3.5
  pip install pymodbus

Usage:
  python3 modbus_slave.py [--host 0.0.0.0] [--port 502] [--cycle-ms 100]
"""

import argparse
import logging
import threading
import time

from pymodbus.datastore import (
    ModbusSequentialDataBlock,
    ModbusDeviceContext,
    ModbusServerContext,
)
from pymodbus.server import StartTcpServer

logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")
log = logging.getLogger("modbus_slave")

# Address map (matches config/hal_modbus_tcp.toml)
COIL_VALVE_EXTEND = 0
COIL_VALVE_RETRACT = 1
DI_SENSOR_HOME = 0
DI_SENSOR_END = 1

NUM_COILS = 16
NUM_DI = 16

# Physics: number of cycles before sensor activates
SENSOR_THRESHOLD = 3


def build_context():
    """Create Modbus data store with coils and discrete inputs."""
    # Use 0-based start address to match Rust side mapping (address 0,1,...).
    coils = ModbusSequentialDataBlock(0, [False] * NUM_COILS)
    discrete_inputs = ModbusSequentialDataBlock(0, [False] * NUM_DI)
    holding_regs = ModbusSequentialDataBlock(0, [0] * 16)
    input_regs = ModbusSequentialDataBlock(0, [0] * 16)

    slave = ModbusDeviceContext(
        di=discrete_inputs,
        co=coils,
        hr=holding_regs,
        ir=input_regs,
    )
    return ModbusServerContext(devices=slave, single=True)


def simulation_loop(context, cycle_ms):
    """Simulate sensor responses based on coil state.

    Simple cylinder physics:
    - valve_extend ON for SENSOR_THRESHOLD cycles → sensor_end HIGH
    - valve_retract ON for SENSOR_THRESHOLD cycles → sensor_home HIGH
    """
    extend_count = 0
    retract_count = 0
    cycle_s = cycle_ms / 1000.0

    # Initial state: cylinder retracted.
    slave = context[0]
    slave.setValues(2, DI_SENSOR_HOME, [True])
    slave.setValues(2, DI_SENSOR_END, [False])

    log.info("Simulation loop started (cycle=%dms, threshold=%d)", cycle_ms, SENSOR_THRESHOLD)

    while True:
        time.sleep(cycle_s)

        slave = context[0]

        # Read coil state using 0-based addressing.
        valve_extend = slave.getValues(1, COIL_VALVE_EXTEND, count=1)[0]
        valve_retract = slave.getValues(1, COIL_VALVE_RETRACT, count=1)[0]

        if valve_extend:
            extend_count += 1
            retract_count = 0
        elif valve_retract:
            retract_count += 1
            extend_count = 0
        else:
            pass

        sensor_home = retract_count >= SENSOR_THRESHOLD or (extend_count == 0 and retract_count == 0)
        sensor_end = extend_count >= SENSOR_THRESHOLD

        # Write discrete inputs using 0-based addressing.
        slave.setValues(2, DI_SENSOR_HOME, [sensor_home])
        slave.setValues(2, DI_SENSOR_END, [sensor_end])

        if extend_count == SENSOR_THRESHOLD or retract_count == SENSOR_THRESHOLD:
            log.info(
                "sensors: home=%s end=%s (extend_cnt=%d retract_cnt=%d)",
                sensor_home, sensor_end, extend_count, retract_count,
            )


def main():
    parser = argparse.ArgumentParser(description="Modbus TCP slave for RustPLC")
    parser.add_argument("--host", default="0.0.0.0", help="Bind address")
    parser.add_argument("--port", type=int, default=502, help="TCP port")
    parser.add_argument("--cycle-ms", type=int, default=100, help="Simulation cycle (ms)")
    args = parser.parse_args()

    context = build_context()

    sim_thread = threading.Thread(
        target=simulation_loop, args=(context, args.cycle_ms), daemon=True
    )
    sim_thread.start()

    log.info("Starting Modbus TCP slave on %s:%d", args.host, args.port)
    StartTcpServer(context=context, address=(args.host, args.port))


if __name__ == "__main__":
    main()
