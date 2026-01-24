use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use color_eyre::eyre::{self, WrapErr};
use modbus_rtu::{Function, Master, Request, Response};

use crate::constants::{STATUS_POLL_REG_COUNT, STATUS_POLL_REG_START};
use crate::data::DeviceStatus;

#[derive(Debug)]
pub enum TransportCommand {
    WriteRegister { register: u16, value: u16 },
    Terminate,
}

#[derive(Debug)]
pub enum TransportEvent {
    Status(DeviceStatus),
    Connection(bool),
    Error(eyre::Report),
}

#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub port: Option<String>,
    pub baud: u32,
    pub address: u8,
    pub poll_interval: Duration,
    pub read_only: bool,
    pub simulate: bool,
}

pub fn spawn_worker(
    config: TransportConfig,
    command_rx: Receiver<TransportCommand>,
    event_tx: Sender<TransportEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let result = if config.simulate {
            run_sim_loop(config, command_rx, &event_tx)
        } else {
            run_serial_loop(config, command_rx, &event_tx)
        };

        if let Err(err) = result {
            let _ = event_tx.send(TransportEvent::Error(err));
        }
    })
}

#[allow(clippy::needless_pass_by_value)]
fn run_serial_loop(
    config: TransportConfig,
    command_rx: Receiver<TransportCommand>,
    event_tx: &Sender<TransportEvent>,
) -> eyre::Result<()> {
    let port = config
        .port
        .as_ref()
        .ok_or_else(|| eyre::eyre!("serial port required"))?;
    let mut master = Master::new_rs485(port, config.baud).wrap_err("open modbus port")?;

    loop {
        match command_rx.recv_timeout(config.poll_interval) {
            Ok(TransportCommand::WriteRegister { register, value }) => {
                if !config.read_only {
                    if write_register(&mut master, &config, register, value).is_err() {
                        event_tx.send(TransportEvent::Connection(false)).ok();
                    }
                }
            }
            Ok(TransportCommand::Terminate) => break,
            Err(RecvTimeoutError::Timeout) => match read_status(&mut master, &config) {
                Ok(Some(status)) => {
                    event_tx.send(TransportEvent::Status(status)).ok();
                    event_tx.send(TransportEvent::Connection(true)).ok();
                }
                Ok(None) | Err(_) => {
                    event_tx.send(TransportEvent::Connection(false)).ok();
                }
            },
            Err(RecvTimeoutError::Disconnected) => {
                return Err(eyre::eyre!("command channel closed"));
            }
        }
    }

    Ok(())
}

fn read_status(
    master: &mut Master,
    config: &TransportConfig,
) -> eyre::Result<Option<DeviceStatus>> {
    let function = Function::ReadHoldingRegisters {
        starting_address: STATUS_POLL_REG_START,
        quantity: STATUS_POLL_REG_COUNT,
    };
    let request = Request::new(config.address, &function, Duration::from_millis(300));
    let response = master.send(&request).wrap_err("read registers")?;
    match response {
        Response::Value(values) => Ok(DeviceStatus::from_registers(values.into_vec())),
        Response::Exception(exception) => Err(eyre::eyre!("device exception: {exception:?}")),
        _ => Err(eyre::eyre!("unexpected response to status read")),
    }
}

fn write_register(
    master: &mut Master,
    config: &TransportConfig,
    register: u16,
    value: u16,
) -> eyre::Result<()> {
    let function = Function::WriteSingleRegister {
        address: register,
        value,
    };
    let request = Request::new(config.address, &function, Duration::from_millis(300));
    let response = master.send(&request).wrap_err("write register")?;
    if response.is_success() {
        Ok(())
    } else {
        Err(eyre::eyre!("write rejected: {response}"))
    }
}

#[cfg(debug_assertions)]
#[allow(clippy::needless_pass_by_value)]
fn run_sim_loop(
    config: TransportConfig,
    command_rx: Receiver<TransportCommand>,
    event_tx: &Sender<TransportEvent>,
) -> eyre::Result<()> {
    crate::sim::run_sim_loop(config, command_rx, event_tx)
}

#[cfg(not(debug_assertions))]
#[allow(clippy::needless_pass_by_value)]
fn run_sim_loop(
    _config: TransportConfig,
    _command_rx: Receiver<TransportCommand>,
    _event_tx: &Sender<TransportEvent>,
) -> eyre::Result<()> {
    Err(eyre::eyre!("simulation not available in release builds"))
}
