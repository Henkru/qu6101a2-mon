use color_eyre::eyre;

use crate::backend::Backend;
use crate::data::DeviceStatus;
use crate::sim::SimState;
use crate::transport::TransportCommand;

pub(crate) struct SimBackend {
    sim: SimState,
}

impl SimBackend {
    pub(crate) fn new() -> Self {
        Self {
            sim: SimState::new(),
        }
    }
}

impl Backend for SimBackend {
    fn poll_status(&mut self) -> eyre::Result<DeviceStatus> {
        Ok(self.sim.tick())
    }

    fn apply_command(&mut self, command: &TransportCommand) -> eyre::Result<()> {
        match command {
            TransportCommand::SetPower(on) => self.sim.set_power(*on),
            TransportCommand::SetTargetFlow(flow) => self.sim.set_target_flow(*flow),
            TransportCommand::Terminate => {}
        }
        Ok(())
    }
}
