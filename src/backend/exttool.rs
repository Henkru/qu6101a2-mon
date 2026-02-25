use std::io::Write;
use std::time::Duration;

use color_eyre::eyre;
use serialport::SerialPort;

use crate::backend::Backend;
use crate::constants::{
    REG_C_FILTER_LIMIT, REG_C_FILTER_TOTAL, REG_M_FILTER_LIMIT, REG_M_FILTER_TOTAL,
    REG_P_FILTER_LIMIT, REG_P_FILTER_TOTAL, REG_REAL_FLOW, REG_SPEED_RPM, REG_STATE,
    REG_STATUS_FLAGS, REG_TARGET_FLOW, REG_TUBE_DIAMETER, STATE_OFF, STATE_ON,
    STATUS_POLL_REG_COUNT, TARGET_FLOW_MAX, TARGET_FLOW_MIN,
};
use crate::data::DeviceStatus;
use crate::rtu::{append_crc, read_exact_with_timeout, validate_crc};
use crate::transport::TransportCommand;

const CMD_READ_STATUS: u8 = 0x67;
const CMD_WRITE_COMMAND: u8 = 0x68;

const IDX_MIN: u8 = 0x10;
const IDX_MAX_EXCLUSIVE: u8 = 0x50;

const STATUS_START: u8 = 0x10;
const STATUS_BYTE_COUNT: u8 = 0x38;

const IDX_REAL_FLOW: u8 = 0x19;
const IDX_P_FILTER_TOTAL: u8 = 0x1A;
const IDX_M_FILTER_TOTAL: u8 = 0x1B;
const IDX_C_FILTER_TOTAL: u8 = 0x1C;
const IDX_SPEED_RPM: u8 = 0x1D;
const IDX_STATE: u8 = 0x1E;
const IDX_TUBE_DIAMETER: u8 = 0x1F;
const IDX_TARGET_FLOW: u8 = 0x28;
const IDX_P_FILTER_LIMIT: u8 = 0x29;
const IDX_M_FILTER_LIMIT: u8 = 0x2A;
const IDX_C_FILTER_LIMIT: u8 = 0x2B;
const IDX_STATUS_FLAGS: u8 = 0x10;

pub(crate) struct ExtToolBackend {
    port: Box<dyn SerialPort>,
    address: u8,
    io_timeout: Duration,
}

impl ExtToolBackend {
    pub(crate) fn new(path: &str, baud: u32, address: u8) -> eyre::Result<Self> {
        let io_timeout = Duration::from_millis(400);
        let port = serialport::new(path, baud)
            .timeout(io_timeout)
            .open()
            .map_err(|err| eyre::eyre!("open serial port: {err}"))?;
        Ok(Self {
            port,
            address,
            io_timeout,
        })
    }

    fn read_status(&mut self) -> eyre::Result<DeviceStatus> {
        let request = build_read_request(self.address, STATUS_START, STATUS_BYTE_COUNT)?;
        self.write_request(&request)?;
        let response = self.read_response_header(CMD_READ_STATUS)?;
        parse_read_response(&response, self.address, STATUS_START)
    }

    fn write_single_register(&mut self, start: u8, value: u16) -> eyre::Result<()> {
        let payload = value.to_be_bytes();
        let request = build_write_request(self.address, start, &payload)?;
        self.write_request(&request)?;
        let response = self.read_response_header(CMD_WRITE_COMMAND)?;
        parse_write_response(&response, self.address, start, 2)
    }

    fn write_request(&mut self, request: &[u8]) -> eyre::Result<()> {
        self.port
            .write_all(request)
            .map_err(|err| eyre::eyre!("write request: {err}"))?;
        self.port
            .flush()
            .map_err(|err| eyre::eyre!("flush request: {err}"))?;
        Ok(())
    }

    fn read_response_header(&mut self, expected_cmd: u8) -> eyre::Result<Vec<u8>> {
        let header = read_exact_with_timeout(&mut *self.port, 3, self.io_timeout)?;
        let address = header[0];
        let command = header[1];

        if address != self.address {
            return Err(eyre::eyre!(
                "unexpected response address: expected 0x{:02X}, got 0x{address:02X}",
                self.address
            ));
        }

        if command == (expected_cmd | 0x80) {
            let tail = read_exact_with_timeout(&mut *self.port, 2, self.io_timeout)?;
            let mut frame = header;
            frame.extend_from_slice(&tail);
            validate_crc(&frame)?;
            let exception = frame[2];
            return Err(eyre::eyre!(
                "device exception for cmd 0x{expected_cmd:02X}: code 0x{exception:02X}"
            ));
        }

        if command != expected_cmd {
            return Err(eyre::eyre!(
                "unexpected response command: expected 0x{expected_cmd:02X}, got 0x{command:02X}"
            ));
        }

        if command == CMD_READ_STATUS {
            let count = usize::from(header[2]);
            let tail = read_exact_with_timeout(&mut *self.port, count + 2, self.io_timeout)?;
            let mut frame = header;
            frame.extend_from_slice(&tail);
            Ok(frame)
        } else {
            let tail = read_exact_with_timeout(&mut *self.port, 3, self.io_timeout)?;
            let mut frame = header;
            frame.extend_from_slice(&tail);
            Ok(frame)
        }
    }
}

impl Backend for ExtToolBackend {
    fn poll_status(&mut self) -> eyre::Result<DeviceStatus> {
        self.read_status()
    }

    fn apply_command(&mut self, command: &TransportCommand) -> eyre::Result<()> {
        match command {
            TransportCommand::SetPower(on) => {
                let state = if *on { STATE_ON } else { STATE_OFF };
                self.write_single_register(IDX_STATE, state)
            }
            TransportCommand::SetTargetFlow(flow) => self.write_single_register(
                IDX_TARGET_FLOW,
                (*flow).clamp(TARGET_FLOW_MIN, TARGET_FLOW_MAX),
            ),
            TransportCommand::Terminate => Ok(()),
        }
    }
}

fn build_read_request(address: u8, start: u8, count: u8) -> eyre::Result<Vec<u8>> {
    validate_range(start, count)?;
    Ok(append_crc(&[address, CMD_READ_STATUS, start, count]))
}

fn build_write_request(address: u8, start: u8, payload: &[u8]) -> eyre::Result<Vec<u8>> {
    let count = u8::try_from(payload.len())
        .map_err(|_| eyre::eyre!("write payload too large: {} bytes", payload.len()))?;
    validate_write_range(start, count)?;
    let mut body = Vec::with_capacity(4 + payload.len());
    body.extend_from_slice(&[address, CMD_WRITE_COMMAND, start, count]);
    body.extend_from_slice(payload);
    Ok(append_crc(&body))
}

fn parse_read_response(
    frame: &[u8],
    expected_addr: u8,
    expected_start: u8,
) -> eyre::Result<DeviceStatus> {
    validate_crc(frame)?;
    if frame.len() < 5 {
        return Err(eyre::eyre!("read response too short"));
    }
    if frame[0] != expected_addr {
        return Err(eyre::eyre!("read response address mismatch"));
    }
    if frame[1] != CMD_READ_STATUS {
        return Err(eyre::eyre!("read response command mismatch"));
    }
    let count = usize::from(frame[2]);
    if frame.len() != count + 5 {
        return Err(eyre::eyre!(
            "read response length mismatch: count={count}, frame_len={}",
            frame.len()
        ));
    }
    let payload = &frame[3..(3 + count)];
    map_status_payload(expected_start, payload)
}

fn parse_write_response(
    frame: &[u8],
    expected_addr: u8,
    expected_start: u8,
    expected_count: u8,
) -> eyre::Result<()> {
    validate_crc(frame)?;
    if frame.len() != 6 {
        return Err(eyre::eyre!(
            "write response length mismatch: got {}",
            frame.len()
        ));
    }
    if frame[0] != expected_addr {
        return Err(eyre::eyre!("write response address mismatch"));
    }
    if frame[1] != CMD_WRITE_COMMAND {
        return Err(eyre::eyre!("write response command mismatch"));
    }
    if frame[2] != expected_start || frame[3] != expected_count {
        return Err(eyre::eyre!(
            "write response echo mismatch: start=0x{:02X}, count={} expected start=0x{:02X}, count={expected_count}",
            frame[2],
            frame[3],
            expected_start,
        ));
    }
    Ok(())
}

fn map_status_payload(start: u8, payload: &[u8]) -> eyre::Result<DeviceStatus> {
    let count = u8::try_from(payload.len())
        .map_err(|_| eyre::eyre!("status payload too large: {} bytes", payload.len()))?;
    validate_range(start, count)?;

    let mut values = Vec::with_capacity(payload.len().div_ceil(2));
    for chunk in payload.chunks_exact(2) {
        values.push(u16::from_be_bytes([chunk[0], chunk[1]]));
    }

    let read_idx = |idx: u8| -> u16 {
        if idx < start {
            return 0;
        }
        let offset = usize::from(idx - start);
        values.get(offset).copied().unwrap_or(0)
    };

    let state = read_idx(IDX_STATE);
    let target_flow = read_idx(IDX_TARGET_FLOW);
    let real_flow = read_idx(IDX_REAL_FLOW);
    let speed_rpm = read_idx(IDX_SPEED_RPM);
    let p_filter_total = read_idx(IDX_P_FILTER_TOTAL);
    let m_filter_total = read_idx(IDX_M_FILTER_TOTAL);
    let c_filter_total = read_idx(IDX_C_FILTER_TOTAL);
    let p_filter_limit = read_idx(IDX_P_FILTER_LIMIT);
    let m_filter_limit = read_idx(IDX_M_FILTER_LIMIT);
    let c_filter_limit = read_idx(IDX_C_FILTER_LIMIT);

    let mut registers = vec![0u16; STATUS_POLL_REG_COUNT as usize];
    registers[REG_STATE as usize] = state;
    registers[REG_TARGET_FLOW as usize] = target_flow;
    registers[REG_STATUS_FLAGS as usize] = read_idx(IDX_STATUS_FLAGS);
    registers[REG_P_FILTER_TOTAL as usize] = p_filter_total;
    registers[REG_M_FILTER_TOTAL as usize] = m_filter_total;
    registers[REG_C_FILTER_TOTAL as usize] = c_filter_total;
    registers[REG_P_FILTER_LIMIT as usize] = p_filter_limit;
    registers[REG_M_FILTER_LIMIT as usize] = m_filter_limit;
    registers[REG_C_FILTER_LIMIT as usize] = c_filter_limit;
    registers[REG_SPEED_RPM as usize] = speed_rpm;
    registers[REG_TUBE_DIAMETER as usize] = read_idx(IDX_TUBE_DIAMETER);
    registers[REG_REAL_FLOW as usize] = real_flow;

    Ok(DeviceStatus {
        state,
        target_flow,
        real_flow,
        speed_rpm,
        p_filter_total,
        m_filter_total,
        c_filter_total,
        p_filter_limit,
        m_filter_limit,
        c_filter_limit,
        registers,
    })
}

fn validate_range(start: u8, count: u8) -> eyre::Result<()> {
    if start < IDX_MIN {
        return Err(eyre::eyre!("invalid exttool range start: 0x{start:02X}"));
    }
    let end = u16::from(start) + u16::from(count);
    if end > u16::from(IDX_MAX_EXCLUSIVE) {
        return Err(eyre::eyre!(
            "invalid exttool range end: start=0x{start:02X}, count={count}"
        ));
    }
    Ok(())
}

fn validate_write_range(start: u8, count: u8) -> eyre::Result<()> {
    if count == 0 {
        return Err(eyre::eyre!("write payload is empty"));
    }
    if !count.is_multiple_of(2) {
        return Err(eyre::eyre!("write payload must be even-length"));
    }
    validate_range(start, count)
}

#[cfg(test)]
mod tests {
    use super::{
        build_read_request, build_write_request, map_status_payload, parse_read_response,
        parse_write_response, CMD_READ_STATUS, CMD_WRITE_COMMAND, IDX_C_FILTER_LIMIT,
        IDX_C_FILTER_TOTAL, IDX_M_FILTER_LIMIT, IDX_M_FILTER_TOTAL, IDX_P_FILTER_LIMIT,
        IDX_P_FILTER_TOTAL, IDX_REAL_FLOW, IDX_SPEED_RPM, IDX_STATE, IDX_TARGET_FLOW,
        STATUS_BYTE_COUNT, STATUS_START,
    };
    use crate::constants::{
        REG_C_FILTER_LIMIT, REG_C_FILTER_TOTAL, REG_M_FILTER_LIMIT, REG_M_FILTER_TOTAL,
        REG_P_FILTER_LIMIT, REG_P_FILTER_TOTAL, REG_REAL_FLOW, REG_SPEED_RPM, REG_STATE,
        REG_STATUS_FLAGS, REG_TARGET_FLOW, REG_TUBE_DIAMETER, STATUS_POLL_REG_COUNT,
    };
    use crate::rtu::append_crc;

    #[test]
    fn builds_read_request_with_expected_shape() {
        let frame = build_read_request(0x01, STATUS_START, STATUS_BYTE_COUNT)
            .expect("request should build");
        assert_eq!(frame[0], 0x01);
        assert_eq!(frame[1], CMD_READ_STATUS);
        assert_eq!(frame[2], STATUS_START);
        assert_eq!(frame[3], STATUS_BYTE_COUNT);
        assert_eq!(frame.len(), 6);
    }

    #[test]
    fn write_request_requires_even_payload() {
        let err = build_write_request(0x01, IDX_TARGET_FLOW, &[0x00])
            .expect_err("odd payload should fail");
        assert!(err.to_string().contains("even-length"));
    }

    #[test]
    fn parses_read_response_and_maps_fields() {
        let mut payload = vec![0u8; usize::from(STATUS_BYTE_COUNT)];
        set_u16(&mut payload, IDX_REAL_FLOW, 64);
        set_u16(&mut payload, IDX_P_FILTER_TOTAL, 15);
        set_u16(&mut payload, IDX_M_FILTER_TOTAL, 25);
        set_u16(&mut payload, IDX_C_FILTER_TOTAL, 35);
        set_u16(&mut payload, IDX_SPEED_RPM, 2500);
        set_u16(&mut payload, IDX_STATE, 1);
        set_u16(&mut payload, IDX_TARGET_FLOW, 70);
        set_u16(&mut payload, IDX_P_FILTER_LIMIT, 200);
        set_u16(&mut payload, IDX_M_FILTER_LIMIT, 1200);
        set_u16(&mut payload, IDX_C_FILTER_LIMIT, 2400);

        let mut frame = vec![0x01, CMD_READ_STATUS, STATUS_BYTE_COUNT];
        frame.extend_from_slice(&payload);
        let frame = append_crc(&frame);

        let status =
            parse_read_response(&frame, 0x01, STATUS_START).expect("response should parse");
        assert_eq!(status.real_flow, 64);
        assert_eq!(status.speed_rpm, 2500);
        assert_eq!(status.state, 1);
        assert_eq!(status.target_flow, 70);
        assert_eq!(status.p_filter_total, 15);
        assert_eq!(status.m_filter_total, 25);
        assert_eq!(status.c_filter_total, 35);
        assert_eq!(status.p_filter_limit, 200);
        assert_eq!(status.m_filter_limit, 1200);
        assert_eq!(status.c_filter_limit, 2400);

        assert_eq!(status.registers.len(), STATUS_POLL_REG_COUNT as usize);
        assert_eq!(status.registers[REG_STATUS_FLAGS as usize], 0);
        assert_eq!(status.registers[REG_STATE as usize], 1);
        assert_eq!(status.registers[REG_TARGET_FLOW as usize], 70);
        assert_eq!(status.registers[REG_REAL_FLOW as usize], 64);
        assert_eq!(status.registers[REG_SPEED_RPM as usize], 2500);
        assert_eq!(status.registers[REG_P_FILTER_TOTAL as usize], 15);
        assert_eq!(status.registers[REG_M_FILTER_TOTAL as usize], 25);
        assert_eq!(status.registers[REG_C_FILTER_TOTAL as usize], 35);
        assert_eq!(status.registers[REG_P_FILTER_LIMIT as usize], 200);
        assert_eq!(status.registers[REG_M_FILTER_LIMIT as usize], 1200);
        assert_eq!(status.registers[REG_C_FILTER_LIMIT as usize], 2400);
        assert_eq!(status.registers[REG_TUBE_DIAMETER as usize], 0);
    }

    #[test]
    fn parse_write_response_validates_echo() {
        let frame = append_crc(&[0x01, CMD_WRITE_COMMAND, IDX_STATE, 0x02]);
        parse_write_response(&frame, 0x01, IDX_STATE, 0x02).expect("echo should match");
    }

    #[test]
    fn parse_write_response_rejects_wrong_echo() {
        let frame = append_crc(&[0x01, CMD_WRITE_COMMAND, IDX_STATE, 0x04]);
        let err = parse_write_response(&frame, 0x01, IDX_STATE, 0x02)
            .expect_err("wrong count should fail");
        assert!(err.to_string().contains("echo mismatch"));
    }

    #[test]
    fn map_status_payload_handles_offsets() {
        let mut payload = vec![0u8; 8];
        payload[0] = 0x00;
        payload[1] = 0x01;
        let status = map_status_payload(IDX_STATE, &payload).expect("mapping should work");
        assert_eq!(status.state, 1);
    }

    fn set_u16(payload: &mut [u8], idx: u8, value: u16) {
        let offset = usize::from(idx - STATUS_START) * 2;
        let [hi, lo] = value.to_be_bytes();
        payload[offset] = hi;
        payload[offset + 1] = lo;
    }
}
