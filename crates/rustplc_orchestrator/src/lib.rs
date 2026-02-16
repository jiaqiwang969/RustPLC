//! Digital twin orchestrator — coordinates all execution modes.
//!
//! Reads a TOML config file, selects the appropriate HAL backend,
//! and provides a factory for creating backend instances.
//!
//! Supported modes:
//! - `sim`: SimBackend (Mode A, CI/unit tests)
//! - `modbus_tcp`: ModbusBackend over TCP (Mode B, virtual factory)
//! - `modbus_rtu`: ModbusBackend over serial (Mode B/C, reserved)
//! - `fpga`: FpgaBackend (Mode C, reserved)

use rustplc_hal::config::{AddressType, DeviceAddress, DeviceMapping, ModbusConfig};
use rustplc_hal::sim::SimBackend;
use rustplc_hal::traits::{HalBackend, HalError};
use rustplc_modbus::ModbusBackend;
use serde::Deserialize;
use std::collections::HashMap;

/// Top-level orchestrator config, deserialized from TOML.
#[derive(Debug, Deserialize)]
pub struct OrchestratorConfig {
    pub mode: ModeConfig,
    #[serde(default)]
    pub modbus: Option<ModbusTcpSection>,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub mapping: MappingSection,
}

#[derive(Debug, Deserialize)]
pub struct ModeConfig {
    #[serde(rename = "type")]
    pub mode_type: String,
}

#[derive(Debug, Deserialize)]
pub struct ModbusTcpSection {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_slave_id")]
    pub slave_id: u8,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    // RTU fields (reserved)
    #[serde(default)]
    pub serial_port: Option<String>,
    #[serde(default)]
    pub baud_rate: Option<u32>,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}
fn default_port() -> u16 {
    502
}
fn default_slave_id() -> u8 {
    1
}
fn default_timeout() -> u64 {
    1000
}

#[derive(Debug, Default, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default = "default_cycle_time")]
    pub cycle_time_ms: u64,
    #[serde(default)]
    pub max_cycles: u64,
}

fn default_cycle_time() -> u64 {
    50
}

#[derive(Debug, Default, Deserialize)]
pub struct MappingSection {
    #[serde(default)]
    pub coils: HashMap<String, u16>,
    #[serde(default)]
    pub discrete_inputs: HashMap<String, u16>,
    #[serde(default)]
    pub holding_registers: HashMap<String, u16>,
    #[serde(default)]
    pub input_registers: HashMap<String, u16>,
}

/// Parsed mode selection.
#[derive(Debug, Clone, PartialEq)]
pub enum HalMode {
    Sim,
    ModbusTcp { host: String, port: u16, slave_id: u8 },
    ModbusRtu { serial_port: String, baud_rate: u32, slave_id: u8 },
    Fpga,
}

impl OrchestratorConfig {
    /// Parse from TOML string.
    pub fn from_toml(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }

    /// Parse from TOML file path.
    pub fn from_file(path: &str) -> Result<Self, OrchestratorError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| OrchestratorError::ConfigError(format!("{path}: {e}")))?;
        Self::from_toml(&content).map_err(|e| OrchestratorError::ConfigError(e.to_string()))
    }

    /// Resolve the mode from config.
    pub fn hal_mode(&self) -> Result<HalMode, OrchestratorError> {
        match self.mode.mode_type.as_str() {
            "sim" => Ok(HalMode::Sim),
            "modbus_tcp" => {
                let m = self.modbus.as_ref().ok_or_else(|| {
                    OrchestratorError::ConfigError("[modbus] section required for modbus_tcp".into())
                })?;
                Ok(HalMode::ModbusTcp {
                    host: m.host.clone(),
                    port: m.port,
                    slave_id: m.slave_id,
                })
            }
            "modbus_rtu" => {
                let m = self.modbus.as_ref().ok_or_else(|| {
                    OrchestratorError::ConfigError("[modbus] section required for modbus_rtu".into())
                })?;
                let serial_port = m.serial_port.clone().ok_or_else(|| {
                    OrchestratorError::ConfigError("modbus.serial_port required for RTU".into())
                })?;
                let baud_rate = m.baud_rate.unwrap_or(9600);
                Ok(HalMode::ModbusRtu {
                    serial_port,
                    baud_rate,
                    slave_id: m.slave_id,
                })
            }
            "fpga" => Ok(HalMode::Fpga),
            other => Err(OrchestratorError::ConfigError(format!(
                "unknown mode type: {other}"
            ))),
        }
    }

    /// Build a DeviceMapping from the [mapping] section.
    fn device_mapping(&self) -> DeviceMapping {
        let mut mapping = HashMap::new();
        for (name, &addr) in &self.mapping.coils {
            mapping.insert(name.clone(), DeviceAddress {
                addr_type: AddressType::Coil,
                address: addr,
            });
        }
        for (name, &addr) in &self.mapping.discrete_inputs {
            mapping.insert(name.clone(), DeviceAddress {
                addr_type: AddressType::DiscreteInput,
                address: addr,
            });
        }
        for (name, &addr) in &self.mapping.holding_registers {
            mapping.insert(name.clone(), DeviceAddress {
                addr_type: AddressType::HoldingRegister,
                address: addr,
            });
        }
        for (name, &addr) in &self.mapping.input_registers {
            mapping.insert(name.clone(), DeviceAddress {
                addr_type: AddressType::InputRegister,
                address: addr,
            });
        }
        let modbus_cfg = self.modbus.as_ref();
        let modbus_port = match (self.mode.mode_type.as_str(), modbus_cfg) {
            ("modbus_tcp", Some(m)) => format!("{}:{}", m.host, m.port),
            ("modbus_rtu", Some(m)) => m.serial_port.clone().unwrap_or_default(),
            _ => String::new(),
        };
        DeviceMapping {
            modbus: ModbusConfig {
                port: modbus_port,
                baud_rate: modbus_cfg.and_then(|m| m.baud_rate).unwrap_or(9600),
                slave_id: modbus_cfg.map(|m| m.slave_id).unwrap_or(1),
                cycle_time_ms: self.runtime.cycle_time_ms,
            },
            mapping,
        }
    }
}

/// Create a HAL backend from orchestrator config.
pub fn create_backend(config: &OrchestratorConfig) -> Result<Box<dyn HalBackend>, OrchestratorError> {
    let mode = config.hal_mode()?;
    match mode {
        HalMode::Sim => Ok(Box::new(SimBackend::new())),
        HalMode::ModbusTcp { host, port, slave_id } => {
            let addr = format!("{host}:{port}")
                .parse()
                .map_err(|e| OrchestratorError::ConfigError(format!("invalid address: {e}")))?;
            let mapping = config.device_mapping();
            let backend = ModbusBackend::connect(addr, slave_id, &mapping)
                .map_err(OrchestratorError::HalError)?;
            Ok(Box::new(backend))
        }
        HalMode::ModbusRtu {
            serial_port,
            baud_rate,
            slave_id,
        } => {
            let mapping = config.device_mapping();
            let backend = ModbusBackend::connect_rtu(&serial_port, baud_rate, slave_id, &mapping)
                .map_err(OrchestratorError::HalError)?;
            Ok(Box::new(backend))
        }
        HalMode::Fpga => Err(OrchestratorError::ConfigError(
            "FPGA backend not yet implemented".into(),
        )),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error("config error: {0}")]
    ConfigError(String),
    #[error("HAL error: {0}")]
    HalError(#[from] HalError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sim_config() {
        let toml = r#"
[mode]
type = "sim"

[runtime]
cycle_time_ms = 50
max_cycles = 0
"#;
        let config = OrchestratorConfig::from_toml(toml).unwrap();
        assert_eq!(config.hal_mode().unwrap(), HalMode::Sim);
        assert_eq!(config.runtime.cycle_time_ms, 50);
    }

    #[test]
    fn parses_modbus_tcp_config() {
        let toml = r#"
[mode]
type = "modbus_tcp"

[modbus]
host = "192.168.100.20"
port = 502
slave_id = 1
timeout_ms = 1000

[runtime]
cycle_time_ms = 50

[mapping.coils]
valve_A = 0
valve_B = 1

[mapping.discrete_inputs]
sensor_A = 0
sensor_B = 1
"#;
        let config = OrchestratorConfig::from_toml(toml).unwrap();
        assert_eq!(
            config.hal_mode().unwrap(),
            HalMode::ModbusTcp {
                host: "192.168.100.20".into(),
                port: 502,
                slave_id: 1,
            }
        );
        assert_eq!(config.mapping.coils.get("valve_A"), Some(&0));
        assert_eq!(config.mapping.discrete_inputs.get("sensor_B"), Some(&1));

        let dm = config.device_mapping();
        assert_eq!(dm.mapping.len(), 4);
    }

    #[test]
    fn parses_modbus_rtu_config() {
        let toml = r#"
[mode]
type = "modbus_rtu"

[modbus]
serial_port = "/dev/ttyUSB0"
baud_rate = 9600
slave_id = 1
"#;
        let config = OrchestratorConfig::from_toml(toml).unwrap();
        assert_eq!(
            config.hal_mode().unwrap(),
            HalMode::ModbusRtu {
                serial_port: "/dev/ttyUSB0".into(),
                baud_rate: 9600,
                slave_id: 1,
            }
        );
    }

    #[test]
    fn rejects_unknown_mode() {
        let toml = r#"
[mode]
type = "quantum"
"#;
        let config = OrchestratorConfig::from_toml(toml).unwrap();
        assert!(config.hal_mode().is_err());
    }

    #[test]
    fn create_backend_returns_sim() {
        let toml = r#"
[mode]
type = "sim"
"#;
        let config = OrchestratorConfig::from_toml(toml).unwrap();
        let backend = create_backend(&config).unwrap();
        // SimBackend: read unknown device returns false
        assert!(!backend.read_digital_input("nonexistent"));
    }

    #[test]
    fn parses_hal_sim_toml_file() {
        let content = std::fs::read_to_string("../../config/hal_sim.toml").unwrap();
        let config = OrchestratorConfig::from_toml(&content).unwrap();
        assert_eq!(config.hal_mode().unwrap(), HalMode::Sim);
    }
}
