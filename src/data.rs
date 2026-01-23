use crate::constants::{
    REG_BAUD_RATE, REG_BEEPER, REG_COMM_ADDRESS, REG_C_FILTER_LIMIT, REG_M_FILTER_LIMIT,
    REG_P_FILTER_LIMIT, REG_REAL_FLOW, REG_SPEED_RPM, REG_STATE, REG_TARGET_FLOW,
    REG_TUBE_DIAMETER, STATUS_POLL_REG_COUNT,
};

#[derive(Debug, Clone)]
pub struct DeviceStatus {
    pub state: u16,
    pub target_flow: u16,
    pub real_flow: u16,
    pub speed_rpm: u16,
    pub p_filter_limit: u16,
    pub m_filter_limit: u16,
    pub c_filter_limit: u16,
    pub registers: Vec<u16>,
}

impl DeviceStatus {
    pub fn from_registers(registers: Vec<u16>) -> Option<Self> {
        if registers.len() < STATUS_POLL_REG_COUNT as usize {
            return None;
        }
        let read_reg = |index: u16| -> u16 { *registers.get(index as usize).unwrap_or(&0) };
        Some(Self {
            state: read_reg(REG_STATE),
            target_flow: read_reg(REG_TARGET_FLOW),
            real_flow: read_reg(REG_REAL_FLOW),
            speed_rpm: read_reg(REG_SPEED_RPM),
            p_filter_limit: read_reg(REG_P_FILTER_LIMIT),
            m_filter_limit: read_reg(REG_M_FILTER_LIMIT),
            c_filter_limit: read_reg(REG_C_FILTER_LIMIT),
            registers,
        })
    }
}

pub fn register_name(index: u16) -> Option<&'static str> {
    match index {
        REG_STATE => Some("State"),
        REG_TARGET_FLOW => Some("Target"),
        REG_P_FILTER_LIMIT => Some("P-Limit"),
        REG_M_FILTER_LIMIT => Some("M-Limit"),
        REG_C_FILTER_LIMIT => Some("C-Limit"),
        REG_COMM_ADDRESS => Some("Address"),
        REG_BAUD_RATE => Some("Baud"),
        REG_BEEPER => Some("Beeper"),
        REG_SPEED_RPM => Some("Speed"),
        REG_TUBE_DIAMETER => Some("Tube"),
        REG_REAL_FLOW => Some("Flow"),
        _ => None,
    }
}
