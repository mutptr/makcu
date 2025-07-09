use std::sync::Arc;

use tokio::net::UdpSocket;

use crate::{Makcu, handler::PacketHandler};

pub struct Server {
    socket: Arc<UdpSocket>,
    handler: Arc<PacketHandler>,
}

impl Server {
    pub async fn new(bind_addr: &str, makcu: Makcu) -> anyhow::Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;
        let handler = PacketHandler::new(makcu);

        tracing::info!(bind_addr, "서버 시작");

        Ok(Self {
            socket: Arc::new(socket),
            handler: Arc::new(handler),
        })
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        loop {
            let mut buf = [0u8; 4 * 1024]; // 4KB
            let (len, from) = self.socket.recv_from(&mut buf).await?;
            let data = buf[..len].to_vec();

            let socket = Arc::clone(&self.socket);
            let handler = Arc::clone(&self.handler);

            tokio::spawn(async move {
                if let Err(e) = Self::handle_client_request(socket, handler, data, from).await {
                    tracing::error!(%from, "{e}");
                }
            });
        }
    }

    async fn handle_client_request(
        socket: Arc<UdpSocket>,
        handler: Arc<PacketHandler>,
        data: Vec<u8>,
        from: std::net::SocketAddr,
    ) -> anyhow::Result<()> {
        if data == b"bridge-client-request" {
            handler.handle_client_request(&socket, from).await
        } else {
            handler.handle_encrypted_packet(data).await
        }
    }
}
