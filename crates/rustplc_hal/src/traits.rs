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
}
