use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};

use color_eyre::eyre;

use crate::constants::{
    REG_C_FILTER_LIMIT, REG_M_FILTER_LIMIT, REG_P_FILTER_LIMIT, REG_REAL_FLOW, REG_SPEED_RPM,
    REG_STATE, REG_TARGET_FLOW, STATE_OFF, STATE_ON, STATUS_POLL_REG_COUNT, TARGET_FLOW_MAX,
    TARGET_FLOW_MIN,
};
use crate::data::DeviceStatus;
use crate::transport::{TransportCommand, TransportConfig, TransportEvent};

#[allow(clippy::needless_pass_by_value)]
pub fn run_sim_loop(
    config: TransportConfig,
    command_rx: Receiver<TransportCommand>,
    event_tx: &Sender<TransportEvent>,
) -> eyre::Result<()> {
    let mut sim = SimState::new();

    loop {
        match command_rx.recv_timeout(config.poll_interval) {
            Ok(TransportCommand::WriteRegister { register, value }) => {
                if !config.read_only {
                    sim.apply_command(register, value);
                }
            }
            Ok(TransportCommand::Terminate) => break,
            Err(RecvTimeoutError::Timeout) => {
                let status = sim.tick();
                event_tx.send(TransportEvent::Status(status)).ok();
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err(eyre::eyre!("command channel closed"));
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct SimState {
    state: u16,
    target_flow: u16,
    real_flow: f64,
    speed_rpm: f64,
    p_filter_limit: u16,
    m_filter_limit: u16,
    c_filter_limit: u16,
}

impl SimState {
    fn new() -> Self {
        Self {
            state: STATE_OFF,
            target_flow: 0,
            real_flow: 0.0,
            speed_rpm: 0.0,
            p_filter_limit: 200,
            m_filter_limit: 1200,
            c_filter_limit: 2400,
        }
    }

    fn apply_command(&mut self, register: u16, value: u16) {
        match register {
            REG_STATE => self.state = value,
            REG_TARGET_FLOW => {
                self.target_flow = value.clamp(TARGET_FLOW_MIN, TARGET_FLOW_MAX);
            }
            _ => {}
        }
    }

    fn tick(&mut self) -> DeviceStatus {
        let mut registers = vec![0u16; STATUS_POLL_REG_COUNT as usize];
        if self.state == STATE_ON {
            let target = f64::from(self.target_flow);
            let delta = target - self.real_flow;
            self.real_flow += delta * 0.2;
        } else {
            self.real_flow *= 0.6;
        }

        self.real_flow = self.real_flow.clamp(0.0, f64::from(TARGET_FLOW_MAX));
        self.speed_rpm = if self.state == STATE_ON {
            self.real_flow * 120.0
        } else {
            0.0
        };

        registers[REG_STATE as usize] = self.state;
        registers[REG_TARGET_FLOW as usize] = self.target_flow;
        registers[REG_REAL_FLOW as usize] = clamp_u16(self.real_flow);
        registers[REG_SPEED_RPM as usize] = clamp_u16(self.speed_rpm);
        registers[REG_P_FILTER_LIMIT as usize] = self.p_filter_limit;
        registers[REG_M_FILTER_LIMIT as usize] = self.m_filter_limit;
        registers[REG_C_FILTER_LIMIT as usize] = self.c_filter_limit;

        DeviceStatus {
            state: self.state,
            target_flow: self.target_flow,
            real_flow: clamp_u16(self.real_flow),
            speed_rpm: clamp_u16(self.speed_rpm),
            p_filter_limit: self.p_filter_limit,
            m_filter_limit: self.m_filter_limit,
            c_filter_limit: self.c_filter_limit,
            registers,
        }
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn clamp_u16(value: f64) -> u16 {
    let clamped = value.round().clamp(0.0, f64::from(u16::MAX));
    clamped as u16
}
