use tokio::sync::{mpsc, oneshot};

use crate::Result;

#[derive(Debug)]
enum Command {
    Write {
        data: Vec<u8>,
        tx: oneshot::Sender<Result<()>>,
    },
    WriteRead {
        data: Vec<u8>,
        tx: oneshot::Sender<Result<String>>,
    },
    Close,
}

pub struct Muxer {
    tx: mpsc::Sender<Command>,
}

impl Muxer {
    pub fn new(mut com: Box<dyn serialport::SerialPort>) -> Self {
        let (tx, mut rx) = mpsc::channel(100);

        // tokio::task::spawn(async move {
        //     while let Some(cmd) = rx.recv().await {
        //         match cmd {
        //             Command::Write { data, tx } => {
        //                 let result = tokio::task::block_in_place(|| {
        //                     com.write_all(&data)?;
        //                     Ok(())
        //                 });
        //                 let _ = tx.send(result);
        //             }
        //             Command::WriteRead { data, tx } => {
        //                 let result = tokio::task::block_in_place(|| {
        //                     com.write_all(&data)?;
        //                     let mut buf = [0; 16];
        //                     let len = com.read(&mut buf)?;
        //                     Ok(String::from_utf8_lossy(&buf[..len]).into_owned())
        //                 });
        //                 let _ = tx.send(result);
        //             }
        //             Command::Close => break,
        //         }
        //     }
        //     drop(com);
        //     tracing::debug!("Muxer 채널 닫힘");
        // });

        std::thread::spawn(move || {
            while let Some(cmd) = rx.blocking_recv() {
                tracing::debug!("Received command");
                let _ = com.clear(serialport::ClearBuffer::All);
                match cmd {
                    Command::Write { data, tx } => {
                        let result = (|| {
                            com.write_all(&data)?;
                            Ok(())
                        })();
                        let _ = tx.send(result);
                    }
                    Command::WriteRead { data, tx } => {
                        let result = (|| {
                            com.write_all(&data)?;

                            let mut res = vec![];
                            while !res.ends_with(b"\r\n>>> ") {
                                let mut buf = [0; 4 * 1024];
                                let len = com.read(&mut buf)?;
                                res.extend_from_slice(&buf[..len]);
                            }
                            Ok(String::from_utf8_lossy(&res).into_owned())
                        })();
                        let _ = tx.send(result);
                    }
                    Command::Close => break,
                }
            }
            drop(com);
            tracing::debug!("Muxer 채널 닫힘");
        });

        Self { tx }
    }

    pub async fn write(&self, data: impl Into<Vec<u8>>) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let cmd = Command::Write {
            data: data.into(),
            tx,
        };
        self.tx.send(cmd).await?;
        rx.await?
    }

    pub async fn write_read(&self, data: impl Into<Vec<u8>>) -> Result<String> {
        let (tx, rx) = oneshot::channel();
        let cmd = Command::WriteRead {
            data: data.into(),
            tx,
        };
        self.tx.send(cmd).await?;
        rx.await?
    }

    pub async fn close(self) -> Result<()> {
        self.tx.send(Command::Close).await?;
        self.tx.closed().await;
        Ok(())
    }
}
