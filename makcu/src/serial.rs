use serialport::SerialPort;

use crate::muxer::Result;

pub fn serial_write(com: &mut Box<dyn SerialPort>, data: &[u8]) -> Result<()> {
    com.write_all(data)?;
    Ok(())
}

pub fn serial_read(com: &mut Box<dyn SerialPort>) -> Result<String> {
    const SUFFIX: &str = "\r\n>>> ";
    let mut res: Vec<u8> = vec![];
    while !res.ends_with(SUFFIX.as_bytes()) {
        let mut buf = [0; 1];
        com.read_exact(&mut buf)?;
        res.push(buf[0]);

        if res.len() > 1024 {
            res.clear();
        }
    }
    Ok(String::from_utf8_lossy(&res)
        .trim_end_matches(SUFFIX)
        .to_owned())
}
