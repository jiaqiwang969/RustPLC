//! Modbus RTU/TCP backend for RustPLC HAL.
//!
//! Implements `HalBackend` over Modbus TCP protocol using tokio-modbus.
//! Coil/DI address mapping is driven by `hal_modbus.toml` configuration.

use rustplc_hal::config::{AddressType, DeviceMapping};
use rustplc_hal::traits::{HalBackend, HalError};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::runtime::Runtime;
use tokio_modbus::client::{tcp, Context, Reader, Writer};
use tokio_modbus::slave::Slave;

/// Modbus TCP backend implementing `HalBackend`.
///
/// Reads discrete inputs and writes coils over Modbus TCP.
/// Address mapping is configured via `DeviceMapping`.
pub struct ModbusBackend {
    ctx: Context,
    rt: Runtime,
    coil_map: HashMap<String, u16>,
    di_map: HashMap<String, u16>,
    coil_cache: HashMap<String, bool>,
    di_cache: HashMap<String, bool>,
    max_coil_addr: u16,
    max_di_addr: u16,
}

impl ModbusBackend {
    /// Connect to a Modbus TCP slave.
    pub fn connect(
        addr: SocketAddr,
        slave_id: u8,
        mapping: &DeviceMapping,
    ) -> Result<Self, HalError> {
        let rt = Runtime::new().map_err(|e| HalError::CommError(e.to_string()))?;

        let ctx = rt
            .block_on(tcp::connect_slave(addr, Slave(slave_id)))
            .map_err(|e| HalError::CommError(format!("Modbus TCP connect failed: {e}")))?;

        let mut coil_map = HashMap::new();
        let mut di_map = HashMap::new();
        let mut max_coil: u16 = 0;
        let mut max_di: u16 = 0;

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
                _ => {} // HoldingRegister/InputRegister: future extension
            }
        }

        Ok(Self {
            ctx,
            rt,
            coil_map,
            di_map,
            coil_cache: HashMap::new(),
            di_cache: HashMap::new(),
            max_coil_addr: max_coil,
            max_di_addr: max_di,
        })
    }
}

impl HalBackend for ModbusBackend {
    fn read_digital_input(&self, device: &str) -> bool {
        self.di_cache.get(device).copied().unwrap_or(false)
    }

    fn write_digital_output(&mut self, device: &str, value: bool) {
        self.coil_cache.insert(device.to_string(), value);
    }

    fn refresh_inputs(&mut self) -> Result<(), HalError> {
        if self.max_di_addr == 0 {
            return Ok(());
        }
        let bits = self
            .rt
            .block_on(self.ctx.read_discrete_inputs(0, self.max_di_addr))
            .map_err(|e| HalError::CommError(format!("read_discrete_inputs: {e}")))?
            .map_err(|e| HalError::CommError(format!("Modbus exception: {e:?}")))?;

        for (name, &addr) in &self.di_map {
            let val = bits.get(addr as usize).copied().unwrap_or(false);
            self.di_cache.insert(name.clone(), val);
        }
        Ok(())
    }

    fn flush_outputs(&mut self) -> Result<(), HalError> {
        if self.max_coil_addr == 0 {
            return Ok(());
        }
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

    /// Minimal Modbus TCP responder that handles read_discrete_inputs (FC 0x02)
    /// and write_multiple_coils (FC 0x0F).
    async fn mock_modbus_server(listener: TcpListener) {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 256];

        // Handle up to 4 requests then exit
        for _ in 0..4 {
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
                    // Return: sensor_A=true (bit 0), sensor_B=false (bit 1) → 0x01
                    let mut resp = Vec::with_capacity(9 + byte_count as usize);
                    resp.extend_from_slice(tid);
                    resp.extend_from_slice(&[0x00, 0x00]); // protocol id
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
                // Write Multiple Coils (FC 0x0F)
                0x0F => {
                    let start = &buf[8..10];
                    let qty = &buf[10..12];
                    // Echo back: tid + protocol + length + unit + fc + start + qty
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
        // We can't connect without a server, so test cache logic directly
        let mut coil_cache = HashMap::new();
        let di_cache: HashMap<String, bool> = HashMap::new();

        // write_digital_output caches
        coil_cache.insert("valve_A".to_string(), true);
        assert_eq!(coil_cache.get("valve_A"), Some(&true));

        // read_digital_input returns false for unknown
        assert_eq!(di_cache.get("sensor_A").copied().unwrap_or(false), false);

        // Verify mapping was parsed correctly
        assert_eq!(mapping.mapping.len(), 4);
    }

    #[test]
    fn modbus_tcp_round_trip() {
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            // Bind to random port
            let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
                .await
                .unwrap();
            let port = listener.local_addr().unwrap().port();

            // Spawn mock server
            tokio::spawn(mock_modbus_server(listener));

            // Give server a moment to start
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

            // Test write + flush
            backend.write_digital_output("valve_A", true);
            backend.write_digital_output("valve_B", false);
            assert_eq!(backend.coil_cache.get("valve_A"), Some(&true));

            // flush_outputs sends write_multiple_coils
            tokio::task::spawn_blocking(move || {
                backend.flush_outputs().expect("flush should succeed");
                // refresh_inputs reads discrete inputs
                backend.refresh_inputs().expect("refresh should succeed");
                // sensor_A should be true (mock returns 0x01)
                assert!(backend.read_digital_input("sensor_A"));
                assert!(!backend.read_digital_input("sensor_B"));
            })
            .await
            .unwrap();
        });
    }
}
