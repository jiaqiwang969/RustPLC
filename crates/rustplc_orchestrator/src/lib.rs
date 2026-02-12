//! Digital twin orchestrator — coordinates all execution modes.
//!
//! Three modes:
//! - Mode A (Pure Virtual): SimBackend, CI/unit tests
//! - Mode B (Virtual Factory): QEMU VMs + Gnucap + jtufem simulations
//! - Mode C (Hardware-in-Loop): FPGA + real sensors/actuators
//!
//! The orchestrator reads a TOML config, selects the appropriate HAL backend,
//! compiles .plc sources, and launches the scan-cycle engine.
//!
//! Status: stub — awaiting Modbus and FPGA backends.
