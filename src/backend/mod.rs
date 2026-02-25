use color_eyre::eyre;

use crate::data::DeviceStatus;
use crate::interface::InterfaceMode;
use crate::transport::{TransportCommand, TransportConfig};

mod exttool;
mod remote;

#[cfg(debug_assertions)]
mod sim;

pub(crate) trait Backend {
    fn poll_status(&mut self) -> eyre::Result<DeviceStatus>;
    fn apply_command(&mut self, command: &TransportCommand) -> eyre::Result<()>;
}

pub(crate) fn build_backend(config: &TransportConfig) -> eyre::Result<Box<dyn Backend + Send>> {
    match config.interface {
        InterfaceMode::Remote => {
            let port = config
                .port
                .as_ref()
                .ok_or_else(|| eyre::eyre!("serial port required"))?;
            let backend = remote::RemoteBackend::new(port, config.baud, config.address)?;
            Ok(Box::new(backend))
        }
        InterfaceMode::Exttool => {
            let port = config
                .port
                .as_ref()
                .ok_or_else(|| eyre::eyre!("serial port required"))?;
            let backend = exttool::ExtToolBackend::new(port, config.baud, config.address)?;
            Ok(Box::new(backend))
        }
        InterfaceMode::Simulation => {
            #[cfg(debug_assertions)]
            {
                Ok(Box::new(sim::SimBackend::new()))
            }
            #[cfg(not(debug_assertions))]
            {
                Err(eyre::eyre!("simulation not available in release builds"))
            }
        }
    }
}
