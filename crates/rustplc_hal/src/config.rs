use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceMapping {
    pub modbus: ModbusConfig,
    pub mapping: HashMap<String, DeviceAddress>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModbusConfig {
    pub port: String,
    pub baud_rate: u32,
    pub slave_id: u8,
    #[serde(default = "default_cycle_time")]
    pub cycle_time_ms: u64,
}

fn default_cycle_time() -> u64 {
    50
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceAddress {
    #[serde(rename = "type")]
    pub addr_type: AddressType,
    pub address: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AddressType {
    Coil,
    DiscreteInput,
    HoldingRegister,
    InputRegister,
}

impl DeviceMapping {
    pub fn from_toml(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }
}
