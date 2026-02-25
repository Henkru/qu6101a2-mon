use std::io::Write;
use std::time::Duration;

use color_eyre::eyre;
use serialport::SerialPort;

use crate::backend::Backend;
use crate::constants::{
    REG_STATE, REG_TARGET_FLOW, STATE_OFF, STATE_ON, STATUS_POLL_REG_COUNT, STATUS_POLL_REG_START,
};
use crate::data::DeviceStatus;
use crate::rtu::{append_crc, read_exact_with_timeout, validate_crc};
use crate::transport::TransportCommand;

const FUNC_READ_HOLDING_REGISTERS: u8 = 0x03;
const FUNC_WRITE_SINGLE_REGISTER: u8 = 0x06;

pub(crate) struct RemoteBackend {
    port: Box<dyn SerialPort>,
    address: u8,
    io_timeout: Duration,
}

impl RemoteBackend {
    pub(crate) fn new(path: &str, baud: u32, address: u8) -> eyre::Result<Self> {
        let io_timeout = Duration::from_millis(400);
        let port = serialport::new(path, baud)
            .timeout(io_timeout)
            .open()
            .map_err(|err| eyre::eyre!("open modbus port: {err}"))?;
        Ok(Self {
            port,
            address,
            io_timeout,
        })
    }

    fn read_status(&mut self) -> eyre::Result<DeviceStatus> {
        let request =
            build_read_holding_request(self.address, STATUS_POLL_REG_START, STATUS_POLL_REG_COUNT)?;
        self.send_request(&request)?;
        let response = self.read_read_holding_response()?;
        let registers =
            parse_read_holding_response(&response, self.address, STATUS_POLL_REG_COUNT)?;
        DeviceStatus::from_registers(registers).ok_or_else(|| eyre::eyre!("missing status"))
    }

    fn write_single_register(&mut self, register: u16, value: u16) -> eyre::Result<()> {
        let request = build_write_single_request(self.address, register, value);
        self.send_request(&request)?;
        let response = self.read_write_single_response()?;
        parse_write_single_response(&response, self.address, register, value)
    }

    fn send_request(&mut self, request: &[u8]) -> eyre::Result<()> {
        self.port
            .write_all(request)
            .map_err(|err| eyre::eyre!("write request: {err}"))?;
        self.port
            .flush()
            .map_err(|err| eyre::eyre!("flush request: {err}"))?;
        Ok(())
    }

    fn read_read_holding_response(&mut self) -> eyre::Result<Vec<u8>> {
        let header = read_exact_with_timeout(&mut *self.port, 3, self.io_timeout)?;
        validate_response_header(self.address, FUNC_READ_HOLDING_REGISTERS, &header)?;
        let byte_count = usize::from(header[2]);
        let tail = read_exact_with_timeout(&mut *self.port, byte_count + 2, self.io_timeout)?;
        let mut frame = header;
        frame.extend_from_slice(&tail);
        Ok(frame)
    }

    fn read_write_single_response(&mut self) -> eyre::Result<Vec<u8>> {
        let header = read_exact_with_timeout(&mut *self.port, 3, self.io_timeout)?;
        validate_response_header(self.address, FUNC_WRITE_SINGLE_REGISTER, &header)?;
        let tail = read_exact_with_timeout(&mut *self.port, 5, self.io_timeout)?;
        let mut frame = header;
        frame.extend_from_slice(&tail);
        Ok(frame)
    }
}

impl Backend for RemoteBackend {
    fn poll_status(&mut self) -> eyre::Result<DeviceStatus> {
        self.read_status()
    }

    fn apply_command(&mut self, command: &TransportCommand) -> eyre::Result<()> {
        let (register, value) = remote_write_for_command(command)
            .ok_or_else(|| eyre::eyre!("unsupported command for remote backend"))?;
        self.write_single_register(register, value)
    }
}

fn build_read_holding_request(address: u8, start: u16, quantity: u16) -> eyre::Result<Vec<u8>> {
    if quantity == 0 {
        return Err(eyre::eyre!("read quantity must be > 0"));
    }
    let mut request = vec![address, FUNC_READ_HOLDING_REGISTERS];
    request.extend_from_slice(&start.to_be_bytes());
    request.extend_from_slice(&quantity.to_be_bytes());
    Ok(append_crc(&request))
}

fn parse_read_holding_response(
    frame: &[u8],
    expected_addr: u8,
    expected_quantity: u16,
) -> eyre::Result<Vec<u16>> {
    validate_crc(frame)?;
    if frame.len() < 5 {
        return Err(eyre::eyre!("read response too short"));
    }
    if frame[0] != expected_addr {
        return Err(eyre::eyre!("read response address mismatch"));
    }
    if frame[1] != FUNC_READ_HOLDING_REGISTERS {
        return Err(eyre::eyre!("read response function mismatch"));
    }

    let byte_count = usize::from(frame[2]);
    if byte_count != usize::from(expected_quantity) * 2 {
        return Err(eyre::eyre!(
            "read response byte count mismatch: got {byte_count}, expected {}",
            usize::from(expected_quantity) * 2
        ));
    }

    if frame.len() != byte_count + 5 {
        return Err(eyre::eyre!(
            "read response length mismatch: got {}, expected {}",
            frame.len(),
            byte_count + 5
        ));
    }

    let payload = &frame[3..(3 + byte_count)];
    if !payload.len().is_multiple_of(2) {
        return Err(eyre::eyre!("read payload is not register-aligned"));
    }

    let mut registers = Vec::with_capacity(payload.len() / 2);
    for chunk in payload.chunks_exact(2) {
        registers.push(u16::from_be_bytes([chunk[0], chunk[1]]));
    }

    Ok(registers)
}

fn build_write_single_request(address: u8, register: u16, value: u16) -> Vec<u8> {
    let mut request = vec![address, FUNC_WRITE_SINGLE_REGISTER];
    request.extend_from_slice(&register.to_be_bytes());
    request.extend_from_slice(&value.to_be_bytes());
    append_crc(&request)
}

fn parse_write_single_response(
    frame: &[u8],
    expected_addr: u8,
    expected_register: u16,
    expected_value: u16,
) -> eyre::Result<()> {
    validate_crc(frame)?;
    if frame.len() != 8 {
        return Err(eyre::eyre!(
            "write response length mismatch: got {}",
            frame.len()
        ));
    }
    if frame[0] != expected_addr {
        return Err(eyre::eyre!("write response address mismatch"));
    }
    if frame[1] != FUNC_WRITE_SINGLE_REGISTER {
        return Err(eyre::eyre!("write response function mismatch"));
    }

    let echoed_register = u16::from_be_bytes([frame[2], frame[3]]);
    let echoed_value = u16::from_be_bytes([frame[4], frame[5]]);
    if echoed_register != expected_register || echoed_value != expected_value {
        return Err(eyre::eyre!(
            "write response echo mismatch: register=0x{echoed_register:04X}, value=0x{echoed_value:04X}"
        ));
    }

    Ok(())
}

fn validate_response_header(
    expected_addr: u8,
    expected_func: u8,
    header: &[u8],
) -> eyre::Result<()> {
    if header.len() != 3 {
        return Err(eyre::eyre!("response header length mismatch"));
    }

    if header[0] != expected_addr {
        return Err(eyre::eyre!(
            "unexpected response address: expected 0x{expected_addr:02X}, got 0x{:02X}",
            header[0]
        ));
    }

    let function = header[1];
    if function == (expected_func | 0x80) {
        let exception = header[2];
        return Err(eyre::eyre!(
            "device exception for function 0x{expected_func:02X}: code 0x{exception:02X}"
        ));
    }

    if function != expected_func {
        return Err(eyre::eyre!(
            "unexpected function code: expected 0x{expected_func:02X}, got 0x{function:02X}"
        ));
    }

    Ok(())
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
    use super::{
        build_read_holding_request, build_write_single_request, parse_read_holding_response,
        parse_write_single_response, remote_write_for_command, FUNC_READ_HOLDING_REGISTERS,
        FUNC_WRITE_SINGLE_REGISTER,
    };
    use crate::constants::{
        REG_STATE, REG_TARGET_FLOW, STATE_OFF, STATE_ON, STATUS_POLL_REG_COUNT,
        STATUS_POLL_REG_START,
    };
    use crate::rtu::append_crc;
    use crate::transport::TransportCommand;

    #[test]
    fn builds_standard_read_holding_request_frame() {
        let frame = build_read_holding_request(0x02, STATUS_POLL_REG_START, STATUS_POLL_REG_COUNT)
            .expect("frame should build");
        assert_eq!(frame[0], 0x02);
        assert_eq!(frame[1], FUNC_READ_HOLDING_REGISTERS);
        assert_eq!(frame[2], 0x00);
        assert_eq!(frame[3], 0x00);
        assert_eq!(frame[4], 0x00);
        assert_eq!(frame[5], STATUS_POLL_REG_COUNT as u8);
        assert_eq!(frame.len(), 8);
    }

    #[test]
    fn parses_standard_read_holding_response_values() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&1u16.to_be_bytes());
        payload.extend_from_slice(&70u16.to_be_bytes());
        let mut frame = vec![0x02, FUNC_READ_HOLDING_REGISTERS, 4];
        frame.extend_from_slice(&payload);
        let frame = append_crc(&frame);

        let regs = parse_read_holding_response(&frame, 0x02, 2).expect("response should parse");
        assert_eq!(regs, vec![1, 70]);
    }

    #[test]
    fn rejects_exception_read_response() {
        let frame = append_crc(&[0x02, FUNC_READ_HOLDING_REGISTERS | 0x80, 0x02]);
        let err = parse_read_holding_response(&frame, 0x02, 2).expect_err("should fail");
        assert!(err.to_string().contains("function mismatch"));
    }

    #[test]
    fn builds_and_parses_write_single_register_frames() {
        let request = build_write_single_request(0x02, REG_TARGET_FLOW, 65);
        assert_eq!(request[0], 0x02);
        assert_eq!(request[1], FUNC_WRITE_SINGLE_REGISTER);

        let response = append_crc(&[
            0x02,
            FUNC_WRITE_SINGLE_REGISTER,
            0x00,
            REG_TARGET_FLOW as u8,
            0x00,
            65,
        ]);
        parse_write_single_response(&response, 0x02, REG_TARGET_FLOW, 65)
            .expect("write response should parse");
    }

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
