//! Modbus RTU/TCP backend for RustPLC HAL.
//!
//! Implements `HalBackend` over Modbus TCP protocol using tokio-modbus.
//! Coil/DI/HR/IR address mapping is driven by orchestrator configuration.

use rustplc_hal::config::{AddressType, DeviceMapping};
use rustplc_hal::traits::{HalBackend, HalError};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::runtime::Runtime;
use tokio_modbus::client::{rtu, tcp, Context, Reader, Writer};
use tokio_modbus::slave::Slave;
use tokio_serial::{DataBits, FlowControl, Parity, SerialPortBuilderExt, StopBits};

const MAX_CONNECT_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 500;

/// Modbus TCP backend implementing `HalBackend`.
///
/// Reads discrete inputs / input registers and writes coils / holding registers
/// over Modbus TCP. Address mapping is configured via `DeviceMapping`.
pub struct ModbusBackend {
    ctx: Context,
    rt: Runtime,
    coil_map: HashMap<String, u16>,
    di_map: HashMap<String, u16>,
    hr_map: HashMap<String, u16>,
    ir_map: HashMap<String, u16>,
    coil_cache: HashMap<String, bool>,
    di_cache: HashMap<String, bool>,
    hr_cache: HashMap<String, u16>,
    ir_cache: HashMap<String, u16>,
    max_coil_addr: u16,
    max_di_addr: u16,
    max_hr_addr: u16,
    max_ir_addr: u16,
}

impl ModbusBackend {
    fn from_ctx(ctx: Context, rt: Runtime, mapping: &DeviceMapping) -> Self {
        let mut coil_map = HashMap::new();
        let mut di_map = HashMap::new();
        let mut hr_map = HashMap::new();
        let mut ir_map = HashMap::new();
        let mut max_coil: u16 = 0;
        let mut max_di: u16 = 0;
        let mut max_hr: u16 = 0;
        let mut max_ir: u16 = 0;

        for (name, dev_addr) in &mapping.mapping {
            match dev_addr.addr_type {
                AddressType::Coil => {
                    coil_map.insert(name.clone(), dev_addr.address);
                    if dev_addr.address >= max_coil {
                        max_coil = dev_addr.address + 1;
                    }
                }
                AddressType::DiscreteInput => {
                    di_map.insert(name.clone(), dev_addr.address);
                    if dev_addr.address >= max_di {
                        max_di = dev_addr.address + 1;
                    }
                }
                AddressType::HoldingRegister => {
                    hr_map.insert(name.clone(), dev_addr.address);
                    if dev_addr.address >= max_hr {
                        max_hr = dev_addr.address + 1;
                    }
                }
                AddressType::InputRegister => {
                    ir_map.insert(name.clone(), dev_addr.address);
                    if dev_addr.address >= max_ir {
                        max_ir = dev_addr.address + 1;
                    }
                }
            }
        }

        log::info!(
            "Modbus mapping: {} coils, {} DI, {} HR, {} IR",
            coil_map.len(),
            di_map.len(),
            hr_map.len(),
            ir_map.len()
        );

        Self {
            ctx,
            rt,
            coil_map,
            di_map,
            hr_map,
            ir_map,
            coil_cache: HashMap::new(),
            di_cache: HashMap::new(),
            hr_cache: HashMap::new(),
            ir_cache: HashMap::new(),
            max_coil_addr: max_coil,
            max_di_addr: max_di,
            max_hr_addr: max_hr,
            max_ir_addr: max_ir,
        }
    }

    /// Connect to a Modbus TCP slave with retry logic.
    pub fn connect(
        addr: SocketAddr,
        slave_id: u8,
        mapping: &DeviceMapping,
    ) -> Result<Self, HalError> {
        let rt = Runtime::new().map_err(|e| HalError::CommError(e.to_string()))?;

        log::info!("Connecting to Modbus TCP slave at {addr} (unit {slave_id})");

        let mut last_err = None;
        let mut ctx_opt = None;
        for attempt in 1..=MAX_CONNECT_RETRIES {
            match rt.block_on(tcp::connect_slave(addr, Slave(slave_id))) {
                Ok(ctx) => {
                    if attempt > 1 {
                        log::info!("Connected on attempt {attempt}");
                    }
                    ctx_opt = Some(ctx);
                    break;
                }
                Err(e) => {
                    log::warn!("Connect attempt {attempt}/{MAX_CONNECT_RETRIES} failed: {e}");
                    last_err = Some(e);
                    if attempt < MAX_CONNECT_RETRIES {
                        std::thread::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS));
                    }
                }
            }
        }
        let ctx = ctx_opt.ok_or_else(|| {
            HalError::CommError(format!(
                "Modbus TCP connect failed after {MAX_CONNECT_RETRIES} attempts: {}",
                last_err.unwrap()
            ))
        })?;

        Ok(Self::from_ctx(ctx, rt, mapping))
    }

    /// Connect to a Modbus RTU slave over a serial port.
    pub fn connect_rtu(
        serial_port: &str,
        baud_rate: u32,
        slave_id: u8,
        mapping: &DeviceMapping,
    ) -> Result<Self, HalError> {
        let rt = Runtime::new().map_err(|e| HalError::CommError(e.to_string()))?;

        log::info!(
            "Connecting to Modbus RTU slave at {serial_port} (baud {baud_rate}, unit {slave_id})"
        );

        let serial_port = serial_port.to_string();
        let ctx = rt.block_on(async move {
            let builder = tokio_serial::new(serial_port, baud_rate)
                .data_bits(DataBits::Eight)
                .parity(Parity::None)
                .stop_bits(StopBits::One)
                .flow_control(FlowControl::None);

            let port = builder
                .open_native_async()
                .map_err(|e| HalError::CommError(format!("open serial port: {e}")))?;

            Ok::<Context, HalError>(rtu::attach_slave(port, Slave(slave_id)))
        })?;

        Ok(Self::from_ctx(ctx, rt, mapping))
    }
}

impl HalBackend for ModbusBackend {
    fn read_digital_input(&self, device: &str) -> bool {
        self.di_cache.get(device).copied().unwrap_or(false)
    }

    fn write_digital_output(&mut self, device: &str, value: bool) {
        self.coil_cache.insert(device.to_string(), value);
    }

    fn read_register(&self, device: &str) -> u16 {
        // Check input registers first, then holding registers
        self.ir_cache
            .get(device)
            .or_else(|| self.hr_cache.get(device))
            .copied()
            .unwrap_or(0)
    }

    fn write_register(&mut self, device: &str, value: u16) {
        self.hr_cache.insert(device.to_string(), value);
    }

    fn refresh_inputs(&mut self) -> Result<(), HalError> {
        // Read discrete inputs (FC 0x02)
        if self.max_di_addr > 0 {
            let bits = self
                .rt
                .block_on(self.ctx.read_discrete_inputs(0, self.max_di_addr))
                .map_err(|e| HalError::CommError(format!("read_discrete_inputs: {e}")))?
                .map_err(|e| HalError::CommError(format!("Modbus exception: {e:?}")))?;

            for (name, &addr) in &self.di_map {
                let val = bits.get(addr as usize).copied().unwrap_or(false);
                self.di_cache.insert(name.clone(), val);
            }
            log::debug!("DI refresh: {} bits read", bits.len());
        }

        // Read input registers (FC 0x04)
        if self.max_ir_addr > 0 {
            let regs = self
                .rt
                .block_on(self.ctx.read_input_registers(0, self.max_ir_addr))
                .map_err(|e| HalError::CommError(format!("read_input_registers: {e}")))?
                .map_err(|e| HalError::CommError(format!("Modbus exception: {e:?}")))?;

            for (name, &addr) in &self.ir_map {
                let val = regs.get(addr as usize).copied().unwrap_or(0);
                self.ir_cache.insert(name.clone(), val);
            }
            log::debug!("IR refresh: {} registers read", regs.len());
        }

        Ok(())
    }

    fn flush_outputs(&mut self) -> Result<(), HalError> {
        // Write coils (FC 0x0F)
        if self.max_coil_addr > 0 {
            let mut coils = vec![false; self.max_coil_addr as usize];
            for (name, &addr) in &self.coil_map {
                if let Some(&val) = self.coil_cache.get(name) {
                    coils[addr as usize] = val;
                }
            }
            self.rt
                .block_on(self.ctx.write_multiple_coils(0, &coils))
                .map_err(|e| HalError::CommError(format!("write_multiple_coils: {e}")))?
                .map_err(|e| HalError::CommError(format!("Modbus exception: {e:?}")))?;
            log::debug!("Coils flush: {} coils written", coils.len());
        }

        // Write holding registers (FC 0x10)
        if self.max_hr_addr > 0 {
            let mut regs = vec![0u16; self.max_hr_addr as usize];
            for (name, &addr) in &self.hr_map {
                if let Some(&val) = self.hr_cache.get(name) {
                    regs[addr as usize] = val;
                }
            }
            self.rt
                .block_on(self.ctx.write_multiple_registers(0, &regs))
                .map_err(|e| HalError::CommError(format!("write_multiple_registers: {e}")))?
                .map_err(|e| HalError::CommError(format!("Modbus exception: {e:?}")))?;
            log::debug!("HR flush: {} registers written", regs.len());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustplc_hal::config::DeviceAddress;
    use std::net::{Ipv4Addr, SocketAddrV4};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn test_mapping() -> DeviceMapping {
        let mut mapping = HashMap::new();
        mapping.insert(
            "valve_A".to_string(),
            DeviceAddress {
                addr_type: AddressType::Coil,
                address: 0,
            },
        );
        mapping.insert(
            "valve_B".to_string(),
            DeviceAddress {
                addr_type: AddressType::Coil,
                address: 1,
            },
        );
        mapping.insert(
            "sensor_A".to_string(),
            DeviceAddress {
                addr_type: AddressType::DiscreteInput,
                address: 0,
            },
        );
        mapping.insert(
            "sensor_B".to_string(),
            DeviceAddress {
                addr_type: AddressType::DiscreteInput,
                address: 1,
            },
        );
        DeviceMapping {
            modbus: rustplc_hal::config::ModbusConfig {
                port: "127.0.0.1:5502".to_string(),
                baud_rate: 9600,
                slave_id: 1,
                cycle_time_ms: 50,
            },
            mapping,
        }
    }

    fn test_mapping_with_registers() -> DeviceMapping {
        let mut dm = test_mapping();
        dm.mapping.insert(
            "pressure".to_string(),
            DeviceAddress {
                addr_type: AddressType::InputRegister,
                address: 0,
            },
        );
        dm.mapping.insert(
            "speed_setpoint".to_string(),
            DeviceAddress {
                addr_type: AddressType::HoldingRegister,
                address: 0,
            },
        );
        dm
    }

    /// Minimal Modbus TCP responder that handles FC 0x02, 0x04, 0x0F, 0x10.
    async fn mock_modbus_server(listener: TcpListener) {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 256];

        for _ in 0..8 {
            let n = stream.read(&mut buf).await.unwrap();
            if n == 0 {
                break;
            }

            let tid = &buf[0..2];
            let unit_id = buf[6];
            let fc = buf[7];

            match fc {
                // Read Discrete Inputs (FC 0x02)
                0x02 => {
                    let count = u16::from_be_bytes([buf[10], buf[11]]);
                    let byte_count = ((count + 7) / 8) as u8;
                    let mut resp = Vec::with_capacity(9 + byte_count as usize);
                    resp.extend_from_slice(tid);
                    resp.extend_from_slice(&[0x00, 0x00]);
                    let len = 3 + byte_count as u16;
                    resp.extend_from_slice(&len.to_be_bytes());
                    resp.push(unit_id);
                    resp.push(fc);
                    resp.push(byte_count);
                    // sensor_A (addr 0) = true, sensor_B (addr 1) = false
                    resp.push(0x01);
                    for _ in 1..byte_count {
                        resp.push(0x00);
                    }
                    stream.write_all(&resp).await.unwrap();
                }
                // Read Input Registers (FC 0x04)
                0x04 => {
                    let count = u16::from_be_bytes([buf[10], buf[11]]);
                    let byte_count = (count * 2) as u8;
                    let mut resp = Vec::with_capacity(9 + byte_count as usize);
                    resp.extend_from_slice(tid);
                    resp.extend_from_slice(&[0x00, 0x00]);
                    let len = 3 + byte_count as u16;
                    resp.extend_from_slice(&len.to_be_bytes());
                    resp.push(unit_id);
                    resp.push(fc);
                    resp.push(byte_count);
                    // pressure (addr 0) = 1024
                    resp.extend_from_slice(&1024u16.to_be_bytes());
                    for _ in 1..count {
                        resp.extend_from_slice(&0u16.to_be_bytes());
                    }
                    stream.write_all(&resp).await.unwrap();
                }
                // Write Multiple Coils (FC 0x0F)
                0x0F => {
                    let start = &buf[8..10];
                    let qty = &buf[10..12];
                    let mut resp = Vec::with_capacity(12);
                    resp.extend_from_slice(tid);
                    resp.extend_from_slice(&[0x00, 0x00, 0x00, 0x06]);
                    resp.push(unit_id);
                    resp.push(fc);
                    resp.extend_from_slice(start);
                    resp.extend_from_slice(qty);
                    stream.write_all(&resp).await.unwrap();
                }
                // Write Multiple Registers (FC 0x10)
                0x10 => {
                    let start = &buf[8..10];
                    let qty = &buf[10..12];
                    let mut resp = Vec::with_capacity(12);
                    resp.extend_from_slice(tid);
                    resp.extend_from_slice(&[0x00, 0x00, 0x00, 0x06]);
                    resp.push(unit_id);
                    resp.push(fc);
                    resp.extend_from_slice(start);
                    resp.extend_from_slice(qty);
                    stream.write_all(&resp).await.unwrap();
                }
                _ => break,
            }
        }
    }

    #[test]
    fn cache_read_write_without_io() {
        let mapping = test_mapping();
        let mut coil_cache = HashMap::new();
        let di_cache: HashMap<String, bool> = HashMap::new();

        coil_cache.insert("valve_A".to_string(), true);
        assert_eq!(coil_cache.get("valve_A"), Some(&true));
        assert_eq!(di_cache.get("sensor_A").copied().unwrap_or(false), false);
        assert_eq!(mapping.mapping.len(), 4);
    }

    #[test]
    fn modbus_tcp_round_trip() {
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
                .await
                .unwrap();
            let port = listener.local_addr().unwrap().port();

            tokio::spawn(mock_modbus_server(listener));
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            let mut mapping = test_mapping();
            mapping.modbus.port = format!("127.0.0.1:{port}");

            let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
            let mut backend =
                tokio::task::spawn_blocking(move || {
                    ModbusBackend::connect(addr, 1, &mapping).expect("connect should succeed")
                })
                .await
                .unwrap();

            backend.write_digital_output("valve_A", true);
            backend.write_digital_output("valve_B", false);
            assert_eq!(backend.coil_cache.get("valve_A"), Some(&true));

            tokio::task::spawn_blocking(move || {
                backend.flush_outputs().expect("flush should succeed");
                backend.refresh_inputs().expect("refresh should succeed");
                assert!(backend.read_digital_input("sensor_A"));
                assert!(!backend.read_digital_input("sensor_B"));
            })
            .await
            .unwrap();
        });
    }

    #[test]
    fn modbus_tcp_register_round_trip() {
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
                .await
                .unwrap();
            let port = listener.local_addr().unwrap().port();

            tokio::spawn(mock_modbus_server(listener));
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            let mut mapping = test_mapping_with_registers();
            mapping.modbus.port = format!("127.0.0.1:{port}");

            let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
            let mut backend =
                tokio::task::spawn_blocking(move || {
                    ModbusBackend::connect(addr, 1, &mapping).expect("connect should succeed")
                })
                .await
                .unwrap();

            // Write holding register
            backend.write_register("speed_setpoint", 500);
            assert_eq!(backend.hr_cache.get("speed_setpoint"), Some(&500));

            tokio::task::spawn_blocking(move || {
                backend.flush_outputs().expect("flush should succeed");
                backend.refresh_inputs().expect("refresh should succeed");
                // Mock returns pressure = 1024
                assert_eq!(backend.read_register("pressure"), 1024);
                // Holding register should also be readable
                assert_eq!(backend.read_register("speed_setpoint"), 500);
            })
            .await
            .unwrap();
        });
    }
}
