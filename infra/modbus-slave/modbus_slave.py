#!/usr/bin/env python3
"""Modbus TCP slave simulator for RustPLC Mode B testing.

Runs inside an ESXi Linux VM (or any Linux host). Exposes coils and
discrete inputs over Modbus TCP port 502. Simulates simple I/O:
  - Coils (FC 0x01/0x05/0x0F): writable outputs (valves, motors)
  - Discrete Inputs (FC 0x02): readable inputs (sensors)

Sensor simulation: when coil 0 (valve_extend) is ON for >2 cycles,
discrete input 1 (sensor_end) goes HIGH. When coil 0 is OFF for >2
cycles, discrete input 0 (sensor_home) goes HIGH.

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
    ModbusSlaveContext,
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


def build_context():
    """Create Modbus data store with coils and discrete inputs."""
    # pymodbus uses 1-based addressing internally
    coils = ModbusSequentialDataBlock(1, [False] * NUM_COILS)
    discrete_inputs = ModbusSequentialDataBlock(1, [False] * NUM_DI)
    holding_regs = ModbusSequentialDataBlock(1, [0] * 16)
    input_regs = ModbusSequentialDataBlock(1, [0] * 16)

    slave = ModbusSlaveContext(
        di=discrete_inputs,
        co=coils,
        hr=holding_regs,
        ir=input_regs,
    )
    return ModbusServerContext(slaves=slave, single=True)


def simulation_loop(context, cycle_ms):
    """Simulate sensor responses based on coil state.

    Simple physics model:
    - valve_extend ON for 2+ cycles → sensor_end = HIGH, sensor_home = LOW
    - valve_extend OFF for 2+ cycles → sensor_home = HIGH, sensor_end = LOW
    """
    extend_count = 0
    retract_count = 0
    cycle_s = cycle_ms / 1000.0

    log.info("Simulation loop started (cycle=%dms)", cycle_ms)

    while True:
        time.sleep(cycle_s)

        slave = context[0]

        # Read coil state (pymodbus 1-based: address+1)
        valve_extend = slave.getValues(1, COIL_VALVE_EXTEND + 1, count=1)[0]
        valve_retract = slave.getValues(1, COIL_VALVE_RETRACT + 1, count=1)[0]

        if valve_extend:
            extend_count += 1
            retract_count = 0
        elif valve_retract:
            retract_count += 1
            extend_count = 0
        else:
            # Neither valve active — hold state
            pass

        # Update discrete inputs based on simulated position
        sensor_home = retract_count >= 2 or (extend_count == 0 and retract_count == 0)
        sensor_end = extend_count >= 2

        # Write discrete inputs (function code 2, pymodbus 1-based)
        slave.setValues(2, DI_SENSOR_HOME + 1, [sensor_home])
        slave.setValues(2, DI_SENSOR_END + 1, [sensor_end])

        if extend_count == 2 or retract_count == 2:
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

    # Start simulation in background thread
    sim_thread = threading.Thread(
        target=simulation_loop, args=(context, args.cycle_ms), daemon=True
    )
    sim_thread.start()

    log.info("Starting Modbus TCP slave on %s:%d", args.host, args.port)
    StartTcpServer(context=context, address=(args.host, args.port))


if __name__ == "__main__":
    main()
