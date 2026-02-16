use thiserror::Error;

#[derive(Debug, Error)]
pub enum HalError {
    #[error("device not found: {0}")]
    DeviceNotFound(String),
    #[error("communication error: {0}")]
    CommError(String),
    #[error("timeout")]
    Timeout,
}

pub trait HalBackend: Send {
    fn read_digital_input(&self, device: &str) -> bool;
    fn write_digital_output(&mut self, device: &str, value: bool);
    fn refresh_inputs(&mut self) -> Result<(), HalError>;
    fn flush_outputs(&mut self) -> Result<(), HalError>;

    /// Read a 16-bit register value (Modbus Input Register / Holding Register).
    fn read_register(&self, _device: &str) -> u16 {
        0
    }
    /// Write a 16-bit register value (Modbus Holding Register).
    fn write_register(&mut self, _device: &str, _value: u16) {}
}

impl HalBackend for Box<dyn HalBackend> {
    fn read_digital_input(&self, device: &str) -> bool {
        (**self).read_digital_input(device)
    }
    fn write_digital_output(&mut self, device: &str, value: bool) {
        (**self).write_digital_output(device, value)
    }
    fn refresh_inputs(&mut self) -> Result<(), HalError> {
        (**self).refresh_inputs()
    }
    fn flush_outputs(&mut self) -> Result<(), HalError> {
        (**self).flush_outputs()
    }
    fn read_register(&self, device: &str) -> u16 {
        (**self).read_register(device)
    }
    fn write_register(&mut self, device: &str, value: u16) {
        (**self).write_register(device, value)
    }
}
