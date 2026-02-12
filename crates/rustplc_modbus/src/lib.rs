//! Modbus RTU/TCP backend for RustPLC HAL.
//!
//! Implements `HalBackend` over Modbus protocol, supporting:
//! - Modbus RTU (serial RS-485) for direct PLC communication
//! - Modbus TCP for networked controllers and QEMU VM bridges
//!
//! Status: stub — awaiting tokio-modbus integration.
