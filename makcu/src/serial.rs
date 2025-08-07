use serialport::SerialPort;

use crate::muxer::Result;

pub fn serial_write(com: &mut Box<dyn SerialPort>, data: &[u8]) -> Result<()> {
    com.write_all(data)?;
    Ok(())
}

pub fn serial_read(com: &mut Box<dyn SerialPort>) -> Result<Vec<String>> {
    const SUFFIX: &str = "\r\n>>> ";
    const MAX_BUFFER_SIZE: usize = 4096;

    let mut buf = Vec::new();
    let mut temp_buf = [0u8; 1024];

    while !buf.ends_with(SUFFIX.as_bytes()) {
        let n = match com.read(&mut temp_buf) {
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(e) => return Err(e.into()),
        };

        buf.extend_from_slice(&temp_buf[..n]);

        if buf.len() > MAX_BUFFER_SIZE {
            buf.clear();
        }
    }

    let str = String::from_utf8_lossy(&buf);

    Ok(str
        .split(SUFFIX)
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_end_matches(SUFFIX).to_owned())
        .collect())
}
