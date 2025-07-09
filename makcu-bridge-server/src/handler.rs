use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use tokio::net::UdpSocket;

use crate::Makcu;
use crate::crypto::CryptoManager;
use crate::protocol::{Decode, Packet, Payload};

pub struct PacketHandler {
    crypto: Arc<CryptoManager>,
    makcu: Makcu,
}

impl PacketHandler {
    pub fn new(makcu: Makcu) -> Self {
        Self {
            crypto: Arc::new(CryptoManager::new()),
            makcu,
        }
    }

    pub async fn handle_client_request(
        &self,
        socket: &UdpSocket,
        from: SocketAddr,
    ) -> anyhow::Result<()> {
        tracing::info!(%from, "ClientHello");
        let response = self.crypto.key();
        socket.send_to(response, from).await?;
        Ok(())
    }

    pub async fn handle_encrypted_packet(&self, data: Vec<u8>) -> anyhow::Result<()> {
        let crypto = Arc::clone(&self.crypto);

        let payload = tokio::task::spawn_blocking(move || -> anyhow::Result<Payload> {
            let packet = Packet::from_slice(&data).context("패킷 디코딩 실패")?;
            let payload = crypto
                .decrypt(&packet.nonce, &packet.data)
                .context("페이로드 복호화 실패")?;
            Payload::from_slice(&payload).context("페이로드 디코딩 실패")
        })
        .await??;

        self.process_payload(payload).await
    }

    async fn process_payload(&self, payload: Payload) -> anyhow::Result<()> {
        match payload {
            Payload::Move(x, y) => {
                tracing::debug!(x, y, "이동");
                self.makcu.mouse_move(x, y).await?;
                Ok(())
            }
            Payload::Unknown => {
                tracing::warn!("알 수 없는 페이로드");
                Ok(())
            }
        }
    }
}
