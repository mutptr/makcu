use axum::{Router, routing::get};
use axum_server::tls_rustls::RustlsConfig;
use tokio::net::UdpSocket;

use crate::{
    Makcu,
    websocket::{AppState, ws_handler},
};

pub struct Server {
    braodcast_address: String,
    websocket_address: String,
    tls_config: RustlsConfig,
    makcu: Makcu,
}

impl Server {
    pub async fn new(
        broadcast_address: impl Into<String>,
        websocket_address: impl Into<String>,
        tls_config: RustlsConfig,
        makcu: Makcu,
    ) -> Self {
        Self {
            braodcast_address: broadcast_address.into(),
            websocket_address: websocket_address.into(),
            tls_config,
            makcu,
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let udp_task = self.run_udp_server();
        let http_task = self.run_http_server();
        let wheel_unlock_task = self.run_wheel_unlock_monitor();

        tokio::try_join!(udp_task, http_task, wheel_unlock_task)?;
        Ok(())
    }

    async fn run_udp_server(&self) -> anyhow::Result<()> {
        tracing::info!("UDP 서버 시작: {}", self.braodcast_address);
        let udp_socket = UdpSocket::bind(&self.braodcast_address).await?;
        loop {
            let mut buf = [0u8; 1024];
            let (len, from) = udp_socket.recv_from(&mut buf).await?;
            let data = &buf[..len];

            if data == b"bridge-client-request" {
                tracing::info!(%from, "client-hello");
                if let Err(e) = udp_socket.send_to(b"ack", &from).await {
                    tracing::error!("UDP 응답 전송 오류: {e}");
                }
            }
        }
    }

    async fn run_http_server(&self) -> anyhow::Result<()> {
        let app_state = AppState::new(self.makcu.clone());

        let app = Router::new()
            .route("/ws", get(ws_handler))
            .with_state(app_state);

        tracing::info!("웹소켓 서버 시작: {}", self.websocket_address);
        axum_server::bind_rustls(self.websocket_address.parse()?, self.tls_config.clone())
            .serve(app.into_make_service())
            .await?;

        Ok(())
    }

    async fn run_wheel_unlock_monitor(&self) -> anyhow::Result<()> {
        let mut key_state = self.makcu.subscribe_buttons();

        loop {
            if let Err(e) = key_state.changed().await {
                tracing::error!("run_wheel_unlock_monitor: {e}");
                break;
            }
            let key = *key_state.borrow();

            if key >> 2 & 1 == 1 {
                _ = self.makcu.release().await;
                _ = self.makcu.unlock_ml().await;
            }
        }

        Ok(())
    }
}
