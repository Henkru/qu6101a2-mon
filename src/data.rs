use crate::constants::{
    REG_BAUD_RATE, REG_BAUD_RATE_LO, REG_BEEPER, REG_CALIBRATION_FACTOR, REG_COMM_ADDRESS,
    REG_C_FILTER_LIMIT, REG_C_FILTER_TOTAL, REG_FLAGS, REG_MODE, REG_M_FILTER_LIMIT,
    REG_M_FILTER_TOTAL, REG_P_FILTER_LIMIT, REG_P_FILTER_TOTAL, REG_REAL_FLOW, REG_SPEED_RPM,
    REG_STATE, REG_STATUS_FLAGS, REG_TARGET_FLOW, REG_THRESHOLD_A, REG_THRESHOLD_B,
    REG_TUBE_DIAMETER, STATUS_POLL_REG_COUNT,
};

#[derive(Debug, Clone)]
pub struct DeviceStatus {
    pub state: u16,
    pub target_flow: u16,
    pub real_flow: u16,
    pub speed_rpm: u16,
    pub p_filter_total: u16,
    pub m_filter_total: u16,
    pub c_filter_total: u16,
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
            p_filter_total: read_reg(REG_P_FILTER_TOTAL),
            m_filter_total: read_reg(REG_M_FILTER_TOTAL),
            c_filter_total: read_reg(REG_C_FILTER_TOTAL),
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
        REG_STATUS_FLAGS => Some("Status"),
        REG_P_FILTER_TOTAL => Some("P-Total"),
        REG_M_FILTER_TOTAL => Some("M-Total"),
        REG_C_FILTER_TOTAL => Some("C-Total"),
        REG_P_FILTER_LIMIT => Some("P-Limit"),
        REG_M_FILTER_LIMIT => Some("M-Limit"),
        REG_C_FILTER_LIMIT => Some("C-Limit"),
        REG_FLAGS => Some("Flags"),
        REG_COMM_ADDRESS => Some("Address"),
        REG_BAUD_RATE_LO => Some("Baud-Lo"),
        REG_BAUD_RATE => Some("Baud-Hi"),
        REG_BEEPER => Some("Beeper"),
        REG_SPEED_RPM => Some("Speed"),
        REG_TUBE_DIAMETER => Some("Tube"),
        REG_THRESHOLD_A => Some("Thresh-A"),
        REG_THRESHOLD_B => Some("Thresh-B"),
        REG_MODE => Some("Mode"),
        REG_CALIBRATION_FACTOR => Some("Cal-Factor"),
        REG_REAL_FLOW => Some("Flow"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::constants::{
        REG_C_FILTER_LIMIT, REG_C_FILTER_TOTAL, REG_M_FILTER_LIMIT, REG_M_FILTER_TOTAL,
        REG_P_FILTER_LIMIT, REG_P_FILTER_TOTAL, STATUS_POLL_REG_COUNT,
    };
    use crate::data::DeviceStatus;

    #[test]
    fn parses_filter_totals_and_limits() {
        let mut registers = vec![0u16; STATUS_POLL_REG_COUNT as usize];
        registers[REG_P_FILTER_TOTAL as usize] = 11;
        registers[REG_M_FILTER_TOTAL as usize] = 22;
        registers[REG_C_FILTER_TOTAL as usize] = 33;
        registers[REG_P_FILTER_LIMIT as usize] = 111;
        registers[REG_M_FILTER_LIMIT as usize] = 222;
        registers[REG_C_FILTER_LIMIT as usize] = 333;

        let status = DeviceStatus::from_registers(registers).expect("status should parse");
        assert_eq!(status.p_filter_total, 11);
        assert_eq!(status.m_filter_total, 22);
        assert_eq!(status.c_filter_total, 33);
        assert_eq!(status.p_filter_limit, 111);
        assert_eq!(status.m_filter_limit, 222);
        assert_eq!(status.c_filter_limit, 333);
    }
}
