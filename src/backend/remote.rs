use std::time::Duration;

use color_eyre::eyre::{self, WrapErr};
use modbus_rtu::{Function, Master, Request, Response};

use crate::backend::Backend;
use crate::constants::{
    REG_STATE, REG_TARGET_FLOW, STATE_OFF, STATE_ON, STATUS_POLL_REG_COUNT, STATUS_POLL_REG_START,
};
use crate::data::DeviceStatus;
use crate::transport::TransportCommand;

pub(crate) struct RemoteBackend {
    master: Master,
    address: u8,
}

impl RemoteBackend {
    pub(crate) fn new(port: &str, baud: u32, address: u8) -> eyre::Result<Self> {
        let master = Master::new_rs485(port, baud).wrap_err("open modbus port")?;
        Ok(Self { master, address })
    }
}

impl Backend for RemoteBackend {
    fn poll_status(&mut self) -> eyre::Result<DeviceStatus> {
        read_status(&mut self.master, self.address)?.ok_or_else(|| eyre::eyre!("missing status"))
    }

    fn apply_command(&mut self, command: &TransportCommand) -> eyre::Result<()> {
        let (register, value) = remote_write_for_command(command)
            .ok_or_else(|| eyre::eyre!("unsupported command for remote backend"))?;
        write_register(&mut self.master, self.address, register, value)
    }
}

fn read_status(master: &mut Master, address: u8) -> eyre::Result<Option<DeviceStatus>> {
    let function = Function::ReadHoldingRegisters {
        starting_address: STATUS_POLL_REG_START,
        quantity: STATUS_POLL_REG_COUNT,
    };
    let request = Request::new(address, &function, Duration::from_millis(300));
    let response = master.send(&request).wrap_err("read registers")?;
    match response {
        Response::Value(values) => Ok(DeviceStatus::from_registers(values.into_vec())),
        Response::Exception(exception) => Err(eyre::eyre!("device exception: {exception:?}")),
        _ => Err(eyre::eyre!("unexpected response to status read")),
    }
}

fn write_register(master: &mut Master, address: u8, register: u16, value: u16) -> eyre::Result<()> {
    let function = Function::WriteSingleRegister {
        address: register,
        value,
    };
    let request = Request::new(address, &function, Duration::from_millis(300));
    let response = master.send(&request).wrap_err("write register")?;
    if response.is_success() {
        Ok(())
    } else {
        Err(eyre::eyre!("write rejected: {response}"))
    }
}

fn remote_write_for_command(command: &TransportCommand) -> Option<(u16, u16)> {
    match command {
        TransportCommand::SetPower(on) => {
            let value = if *on { STATE_ON } else { STATE_OFF };
            Some((REG_STATE, value))
        }
        TransportCommand::SetTargetFlow(flow) => Some((REG_TARGET_FLOW, *flow)),
        TransportCommand::Terminate => None,
    }
}

#[cfg(test)]
mod tests {
    use super::remote_write_for_command;
    use crate::constants::{REG_STATE, REG_TARGET_FLOW, STATE_OFF, STATE_ON};
    use crate::transport::TransportCommand;

    #[test]
    fn maps_power_command_to_state_register() {
        assert_eq!(
            remote_write_for_command(&TransportCommand::SetPower(true)),
            Some((REG_STATE, STATE_ON))
        );
        assert_eq!(
            remote_write_for_command(&TransportCommand::SetPower(false)),
            Some((REG_STATE, STATE_OFF))
        );
    }

    #[test]
    fn maps_target_flow_command_to_target_register() {
        assert_eq!(
            remote_write_for_command(&TransportCommand::SetTargetFlow(75)),
            Some((REG_TARGET_FLOW, 75))
        );
    }

    #[test]
    fn terminate_has_no_register_mapping() {
        assert_eq!(remote_write_for_command(&TransportCommand::Terminate), None);
    }
}
