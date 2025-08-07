use serialport::SerialPort;
use tokio::sync::{mpsc, oneshot, watch};

use crate::serial::{serial_read, serial_write};

#[derive(Debug)]
enum Command {
    Write {
        data: Vec<u8>,
    },
    WriteRead {
        data: Vec<u8>,
        tx: oneshot::Sender<String>,
    },
    Close,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io timeout")]
    IoTimeout,
    #[error("channel closed")]
    ChannelClosed,
    #[error(transparent)]
    Io(std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::TimedOut => Error::IoTimeout,
            _ => Error::Io(e),
        }
    }
}

impl From<mpsc::error::SendError<Command>> for Error {
    fn from(_: mpsc::error::SendError<Command>) -> Self {
        Error::ChannelClosed
    }
}

impl From<oneshot::error::RecvError> for Error {
    fn from(_: oneshot::error::RecvError) -> Self {
        Error::ChannelClosed
    }
}

#[derive(Clone)]
pub(crate) struct Muxer {
    tx: mpsc::Sender<Command>,
    watch_tx: watch::Sender<u8>,
}

impl Muxer {
    pub fn new(com: Box<dyn SerialPort>) -> Self {
        let (tx, rx) = mpsc::channel(32);
        let (watch_tx, _) = watch::channel(0);

        spawn_serial_worker(com, rx, watch_tx.clone());

        Self { tx, watch_tx }
    }

    pub async fn write(&self, data: impl Into<Vec<u8>>) -> Result<()> {
        self.tx.send(Command::Write { data: data.into() }).await?;
        Ok(())
    }

    pub async fn write_read(&self, data: impl Into<Vec<u8>>) -> Result<String> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Command::WriteRead {
                data: data.into(),
                tx,
            })
            .await?;

        let response = rx.await?;
        Ok(response)
    }

    pub fn subscribe_buttons(&self) -> watch::Receiver<u8> {
        self.watch_tx.subscribe()
    }

    pub async fn close(&self) -> Result<()> {
        self.tx.send(Command::Close).await?;
        self.tx.closed().await;
        Ok(())
    }
}

fn spawn_serial_worker(
    mut com: Box<dyn SerialPort>,
    mut rx: mpsc::Receiver<Command>,
    watch_tx: watch::Sender<u8>,
) {
    std::thread::spawn(move || {
        loop {
            match run_serial_loop(&mut com, &mut rx, &watch_tx) {
                Ok(()) => continue,
                Err(Error::IoTimeout) => continue,
                Err(e) => {
                    tracing::debug!("run_serial_loop error: {e:?}");
                    break;
                }
            }
        }

        drop(com);
        tracing::debug!("Serial worker closed");
    });
}

fn poll_buttons(
    com: &mut Box<dyn serialport::SerialPort>,
    watch_tx: &watch::Sender<u8>,
) -> Result<()> {
    let responses = serial_read(com)?;
    for response in responses {
        let bytes = response.as_bytes();
        tracing::debug!("serial_read: {response}");
        // km.<byte>
        if bytes.len() == 4 && bytes.starts_with(b"km.") && bytes[3] < 32 {
            tracing::debug!("buttons: {}", bytes[3]);
            _ = watch_tx.send(bytes[3]);
        }
    }
    Ok(())
}

fn handle_command(com: &mut Box<dyn serialport::SerialPort>, cmd: Command) -> Result<()> {
    match cmd {
        Command::Write { data } => serial_write(com, &data),
        Command::WriteRead { data, tx } => {
            serial_write(com, &data)?;
            let read_results = serial_read(com)?;
            let read_result = read_results.into_iter().next().unwrap_or_default();
            tracing::debug!("Read data: {read_result}");
            _ = tx.send(read_result);
            Ok(())
        }
        Command::Close => {
            tracing::debug!("Command::Close");
            Err(Error::ChannelClosed)
        }
    }
}

fn run_serial_loop(
    com: &mut Box<dyn serialport::SerialPort>,
    rx: &mut mpsc::Receiver<Command>,
    watch_tx: &watch::Sender<u8>,
) -> Result<()> {
    match rx.try_recv() {
        Ok(cmd) => handle_command(com, cmd),
        Err(mpsc::error::TryRecvError::Empty) => poll_buttons(com, watch_tx),
        Err(mpsc::error::TryRecvError::Disconnected) => Err(Error::ChannelClosed),
    }
}
