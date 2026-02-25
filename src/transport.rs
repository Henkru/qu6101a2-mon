use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use color_eyre::eyre;

use crate::backend::build_backend;
use crate::data::DeviceStatus;
use crate::interface::InterfaceMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportCommand {
    SetPower(bool),
    SetTargetFlow(u16),
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
    pub interface: InterfaceMode,
}

pub fn spawn_worker(
    config: TransportConfig,
    command_rx: Receiver<TransportCommand>,
    event_tx: Sender<TransportEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let result = run_worker_loop(config, command_rx, &event_tx);

        if let Err(err) = result {
            let _ = event_tx.send(TransportEvent::Error(err));
        }
    })
}

#[allow(clippy::needless_pass_by_value)]
fn run_worker_loop(
    config: TransportConfig,
    command_rx: Receiver<TransportCommand>,
    event_tx: &Sender<TransportEvent>,
) -> eyre::Result<()> {
    let mut backend = build_backend(&config)?;

    loop {
        match command_rx.recv_timeout(config.poll_interval) {
            Ok(TransportCommand::SetPower(on)) => {
                if !config.read_only
                    && backend
                        .apply_command(&TransportCommand::SetPower(on))
                        .is_err()
                {
                    event_tx.send(TransportEvent::Connection(false)).ok();
                }
            }
            Ok(TransportCommand::SetTargetFlow(value)) => {
                if !config.read_only
                    && backend
                        .apply_command(&TransportCommand::SetTargetFlow(value))
                        .is_err()
                {
                    event_tx.send(TransportEvent::Connection(false)).ok();
                }
            }
            Ok(TransportCommand::Terminate) => break,
            Err(RecvTimeoutError::Timeout) => match backend.poll_status() {
                Ok(status) => {
                    event_tx.send(TransportEvent::Status(status)).ok();
                    event_tx.send(TransportEvent::Connection(true)).ok();
                }
                Err(_) => {
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
