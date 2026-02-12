use crate::traits::{HalBackend, HalError};
use std::collections::HashMap;

pub struct SimBackend {
    inputs: HashMap<String, bool>,
    outputs: HashMap<String, bool>,
}

impl SimBackend {
    pub fn new() -> Self {
        Self {
            inputs: HashMap::new(),
            outputs: HashMap::new(),
        }
    }

    pub fn set_input(&mut self, device: &str, value: bool) {
        self.inputs.insert(device.to_string(), value);
    }

    pub fn get_output(&self, device: &str) -> Option<bool> {
        self.outputs.get(device).copied()
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
}
