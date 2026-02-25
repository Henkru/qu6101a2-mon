pub const REG_STATE: u16 = 0x0000;
pub const REG_TARGET_FLOW: u16 = 0x0001;
pub const REG_STATUS_FLAGS: u16 = 0x0002;
pub const REG_P_FILTER_TOTAL: u16 = 0x0003;
pub const REG_M_FILTER_TOTAL: u16 = 0x0004;
pub const REG_C_FILTER_TOTAL: u16 = 0x0005;
pub const REG_P_FILTER_LIMIT: u16 = 0x0006;
pub const REG_M_FILTER_LIMIT: u16 = 0x0007;
pub const REG_C_FILTER_LIMIT: u16 = 0x0008;
pub const REG_FLAGS: u16 = 0x0009;
pub const REG_COMM_ADDRESS: u16 = 0x000A;
pub const REG_BAUD_RATE_LO: u16 = 0x000B;
pub const REG_BAUD_RATE: u16 = 0x000C;
pub const REG_BEEPER: u16 = 0x000D;
pub const REG_SPEED_RPM: u16 = 0x000E;
pub const REG_TUBE_DIAMETER: u16 = 0x000F;
pub const REG_THRESHOLD_A: u16 = 0x0010;
pub const REG_THRESHOLD_B: u16 = 0x0011;
pub const REG_MODE: u16 = 0x0012;
pub const REG_CALIBRATION_FACTOR: u16 = 0x0013;
pub const REG_REAL_FLOW: u16 = 0x0014;

pub const STATE_OFF: u16 = 0;
pub const STATE_ON: u16 = 1;

pub const TARGET_FLOW_MIN: u16 = 30;
pub const TARGET_FLOW_MAX: u16 = 100;

pub const STATUS_POLL_REG_START: u16 = 0x0000;
pub const STATUS_POLL_REG_COUNT: u16 = 0x0018;
