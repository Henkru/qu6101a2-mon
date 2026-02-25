use color_eyre::eyre;

pub fn crc16_modbus(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for byte in data {
        crc ^= u16::from(*byte);
        for _ in 0..8 {
            if crc & 1 == 1 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

pub fn append_crc(frame: &[u8]) -> Vec<u8> {
    let crc = crc16_modbus(frame);
    let mut out = Vec::with_capacity(frame.len() + 2);
    out.extend_from_slice(frame);
    out.push((crc & 0x00FF) as u8);
    out.push((crc >> 8) as u8);
    out
}

pub fn validate_crc(frame: &[u8]) -> eyre::Result<()> {
    if frame.len() < 4 {
        return Err(eyre::eyre!("rtu frame too short"));
    }
    let body_len = frame.len() - 2;
    let expected = crc16_modbus(&frame[..body_len]);
    let seen = u16::from(frame[body_len]) | (u16::from(frame[body_len + 1]) << 8);
    if expected != seen {
        return Err(eyre::eyre!(
            "invalid frame crc: expected 0x{expected:04X}, got 0x{seen:04X}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{append_crc, crc16_modbus, validate_crc};

    #[test]
    fn crc_matches_known_vector() {
        let crc = crc16_modbus(b"123456789");
        assert_eq!(crc, 0x4B37);
    }

    #[test]
    fn append_and_validate_crc_roundtrip() {
        let frame = append_crc(&[0x01, 0x67, 0x10, 0x38]);
        validate_crc(&frame).expect("crc should validate");
    }

    #[test]
    fn validate_crc_fails_for_tampered_frame() {
        let mut frame = append_crc(&[0x01, 0x68, 0x1E, 0x02, 0x00, 0x01]);
        frame[3] ^= 0xFF;
        let err = validate_crc(&frame).expect_err("crc should fail");
        assert!(err.to_string().contains("invalid frame crc"));
    }
}
