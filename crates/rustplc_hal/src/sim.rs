use crate::traits::{HalBackend, HalError};
use std::collections::HashMap;

pub struct SimBackend {
    inputs: HashMap<String, bool>,
    outputs: HashMap<String, bool>,
    registers_in: HashMap<String, u16>,
    registers_out: HashMap<String, u16>,
}

impl SimBackend {
    pub fn new() -> Self {
        Self {
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            registers_in: HashMap::new(),
            registers_out: HashMap::new(),
        }
    }

    pub fn set_input(&mut self, device: &str, value: bool) {
        self.inputs.insert(device.to_string(), value);
    }

    pub fn get_output(&self, device: &str) -> Option<bool> {
        self.outputs.get(device).copied()
    }

    pub fn set_register_input(&mut self, device: &str, value: u16) {
        self.registers_in.insert(device.to_string(), value);
    }

    pub fn get_register_output(&self, device: &str) -> Option<u16> {
        self.registers_out.get(device).copied()
    }
}

impl Default for SimBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl HalBackend for SimBackend {
    fn read_digital_input(&self, device: &str) -> bool {
        self.inputs.get(device).copied().unwrap_or(false)
    }

    fn write_digital_output(&mut self, device: &str, value: bool) {
        self.outputs.insert(device.to_string(), value);
    }

    fn refresh_inputs(&mut self) -> Result<(), HalError> {
        Ok(())
    }

    fn flush_outputs(&mut self) -> Result<(), HalError> {
        Ok(())
    }

    fn read_register(&self, device: &str) -> u16 {
        self.registers_in.get(device).copied().unwrap_or(0)
    }

    fn write_register(&mut self, device: &str, value: u16) {
        self.registers_out.insert(device.to_string(), value);
    }
}
