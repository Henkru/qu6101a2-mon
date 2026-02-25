use std::io;
use std::io::Read;
use std::time::{Duration, Instant};

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

pub fn read_exact_with_timeout(
    reader: &mut dyn Read,
    size: usize,
    timeout: Duration,
) -> eyre::Result<Vec<u8>> {
    let mut buffer = vec![0u8; size];
    let mut read_total = 0usize;
    let deadline = Instant::now() + timeout;

    while read_total < size {
        if Instant::now() > deadline {
            return Err(eyre::eyre!(
                "read timeout while waiting for {} bytes (got {})",
                size,
                read_total
            ));
        }

        match reader.read(&mut buffer[read_total..]) {
            Ok(0) => {}
            Ok(read_now) => read_total += read_now,
            Err(err) if err.kind() == io::ErrorKind::TimedOut => {}
            Err(err) => return Err(eyre::eyre!("read failed: {err}")),
        }
    }

    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::io::Read;
    use std::time::Duration;

    use super::{append_crc, crc16_modbus, read_exact_with_timeout, validate_crc};

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

    #[test]
    fn read_exact_with_timeout_reads_requested_bytes() {
        let mut reader = io::Cursor::new(vec![1, 2, 3, 4]);
        let out = read_exact_with_timeout(&mut reader, 4, Duration::from_millis(50))
            .expect("read should succeed");
        assert_eq!(out, vec![1, 2, 3, 4]);
    }

    #[test]
    fn read_exact_with_timeout_fails_when_source_stalls() {
        struct EmptyReader;
        impl Read for EmptyReader {
            fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                Ok(0)
            }
        }

        let mut reader = EmptyReader;
        let err = read_exact_with_timeout(&mut reader, 1, Duration::from_millis(1))
            .expect_err("read should time out");
        assert!(err.to_string().contains("timeout"));
    }
}
