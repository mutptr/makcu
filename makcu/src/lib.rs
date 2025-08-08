use std::{marker::PhantomData, time::Duration};

use tokio::sync::watch;

use crate::muxer::Muxer;

mod muxer;
mod serial;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("device not found")]
    DeviceNotFound,
    #[error(transparent)]
    Muxer(#[from] muxer::Error),
    #[error(transparent)]
    Serial(#[from] serialport::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn find_device() -> Result<String> {
    const VID: u16 = 0x1A86;
    const PID: u16 = 0x55D3;

    let makcu_port =
        serialport::available_ports()?
            .into_iter()
            .find(|port| match &port.port_type {
                serialport::SerialPortType::UsbPort(usb) => usb.vid == VID && usb.pid == PID,
                _ => false,
            });

    makcu_port
        .map(|port| port.port_name)
        .ok_or(Error::DeviceNotFound)
}

pub fn check_version(version: &str) -> bool {
    return version == "km.MAKCU";
}

pub trait BaudRate {
    const BAUD_RATE: u32;
}

#[derive(Clone)]
pub struct Normal;

#[derive(Clone)]
pub struct HighSpeed;

impl BaudRate for Normal {
    const BAUD_RATE: u32 = 115_200;
}

impl BaudRate for HighSpeed {
    const BAUD_RATE: u32 = 4_000_000;
}

#[derive(Clone)]
pub struct Makcu<B: BaudRate> {
    port_name: String,
    muxer: Muxer,
    _b: PhantomData<B>,
}

impl<B: BaudRate> Makcu<B> {
    fn from_port(port_name: impl Into<String>) -> Result<Self> {
        let port_name = port_name.into();
        let builder = serialport::new(&port_name, B::BAUD_RATE).timeout(Duration::from_millis(1));
        tracing::debug!(port_name, baud_rate = B::BAUD_RATE, "시리얼 연결");
        let com = builder.open()?;
        let muxer = Muxer::new(com);

        Ok(Self {
            port_name,
            muxer,
            _b: PhantomData,
        })
    }

    pub async fn close(self) -> Result<()> {
        self.muxer.close().await?;
        Ok(())
    }

    pub fn port_name(&self) -> &str {
        &self.port_name
    }

    pub async fn version(&self) -> Result<String> {
        let command = "km.version()\r";

        let res = self.muxer.write_read(command).await?;
        Ok(res)
    }

    pub async fn mouse_move(&self, x: i32, y: i32) -> Result<()> {
        let x = x.clamp(i8::MIN as i32, i8::MAX as i32);
        let y = y.clamp(i8::MIN as i32, i8::MAX as i32);
        let command = format!("km.move({x},{y})\r");
        self.muxer.write(command).await?;
        Ok(())
    }

    pub async fn click(&self) -> Result<()> {
        self.press().await?;
        let sleep_time = rand::random_range(30..70);
        tokio::time::sleep(Duration::from_millis(sleep_time)).await;
        self.release().await?;
        Ok(())
    }

    pub async fn press(&self) -> Result<()> {
        let down = "km.left(1)\r";
        self.muxer.write(down).await?;
        Ok(())
    }

    pub async fn release(&self) -> Result<()> {
        let up: &'static str = "km.left(0)\r";
        self.muxer.write(up).await?;
        Ok(())
    }

    pub async fn lock_ml(&self) -> Result<()> {
        let command = "km.lock_ml(1)\r";
        self.muxer.write(command).await?;
        Ok(())
    }

    pub async fn unlock_ml(&self) -> Result<()> {
        let command = "km.lock_ml(0)\r";
        self.muxer.write(command).await?;
        Ok(())
    }

    pub async fn lock_ms1(&self) -> Result<()> {
        let command = "km.lock_ms1(1)\r";
        self.muxer.write(command).await?;
        Ok(())
    }

    pub async fn unlock_ms1(&self) -> Result<()> {
        let command = "km.lock_ms1(0)\r";
        self.muxer.write(command).await?;
        Ok(())
    }

    pub async fn enable_buttons(&self) -> Result<()> {
        let command = "km.buttons(1)\r";
        self.muxer.write(command).await?;
        Ok(())
    }

    pub fn subscribe_buttons(&self) -> watch::Receiver<u8> {
        self.muxer.subscribe_buttons()
    }
}

impl Makcu<Normal> {
    pub fn normal() -> Result<Self> {
        let port_name = find_device()?;
        Makcu::from_port(port_name)
    }

    pub async fn enable_high_speed_mode(self) -> Result<Makcu<HighSpeed>> {
        let command = [
            0xDE, 0xAD, // magic
            0x05, 0x00, // size
            0xA5, // command
            0x00, 0x09, 0x3D, 0x00, // baud rate 4M (little-endian)
        ];
        self.muxer.write(command).await?;
        self.muxer.close().await?;

        Makcu::from_port(self.port_name)
    }
}

impl Makcu<HighSpeed> {
    pub fn high_speed() -> Result<Self> {
        let port_name = find_device()?;
        Makcu::from_port(port_name)
    }
}
