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

pub trait HalBackend {
    fn read_digital_input(&self, device: &str) -> bool;
    fn write_digital_output(&mut self, device: &str, value: bool);
    fn refresh_inputs(&mut self) -> Result<(), HalError>;
    fn flush_outputs(&mut self) -> Result<(), HalError>;
}
