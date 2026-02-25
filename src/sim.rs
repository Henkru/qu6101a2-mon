use crate::constants::{
    REG_C_FILTER_LIMIT, REG_M_FILTER_LIMIT, REG_P_FILTER_LIMIT, REG_REAL_FLOW, REG_SPEED_RPM,
    REG_STATE, REG_TARGET_FLOW, STATE_OFF, STATE_ON, STATUS_POLL_REG_COUNT, TARGET_FLOW_MAX,
    TARGET_FLOW_MIN,
};
use crate::data::DeviceStatus;

#[derive(Debug, Clone)]
pub struct SimState {
    state: u16,
    target_flow: u16,
    real_flow: f64,
    speed_rpm: f64,
    p_filter_limit: u16,
    m_filter_limit: u16,
    c_filter_limit: u16,
}

impl SimState {
    pub fn new() -> Self {
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

    pub fn set_power(&mut self, on: bool) {
        self.state = if on { STATE_ON } else { STATE_OFF };
    }

    pub fn set_target_flow(&mut self, value: u16) {
        self.target_flow = value.clamp(TARGET_FLOW_MIN, TARGET_FLOW_MAX);
    }

    pub fn tick(&mut self) -> DeviceStatus {
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
