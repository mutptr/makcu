pub type Makcu = makcu::Makcu<makcu::HighSpeed>;

use std::time::Duration;

use axum_server::tls_rustls::RustlsConfig;

use crate::server::Server;

mod emulator;
mod server;
mod websocket;

const BROADCAST_ADDRESS: &str = "0.0.0.0:39026";
const WEBSOCKET_ADDRESS: &str = "0.0.0.0:39027";
const CERT_PATH: &str = "cert.pem";
const KEY_PATH: &str = "key.pem";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(tracing::Level::INFO.into())
            }),
        )
        .with_timer(tracing_subscriber::fmt::time::ChronoLocal::new(
            "%Y-%m-%d %H:%M:%S".to_owned(),
        ))
        .init();

    if let Err(err) = start().await {
        let mut error_chain = err.to_string();
        let mut source = err.source();
        while let Some(err) = source {
            error_chain = format!("{error_chain}: {err}");
            source = err.source();
        }
        tracing::error!("{error_chain}");
    }
}

async fn start() -> anyhow::Result<()> {
    let server = create_server().await?;
    server.run().await?;
    Ok(())
}

async fn create_server() -> anyhow::Result<Server> {
    let makcu = connect_makcu().await?;
    makcu.enable_buttons().await?;

    let tls_config = RustlsConfig::from_pem_file(CERT_PATH, KEY_PATH).await?;

    Ok(Server::new(BROADCAST_ADDRESS, WEBSOCKET_ADDRESS, tls_config, makcu).await)
}

async fn connect_makcu() -> anyhow::Result<Makcu> {
    let makcu = makcu::Makcu::normal()?;
    tracing::info!("{} 연결됨", makcu.port_name());

    let version_result = makcu.version().await;
    let makcu = match version_result {
        Ok(_) => {
            tracing::info!("고성능 모드 활성화");
            let r = makcu.enable_high_speed_mode().await;
            tokio::time::sleep(Duration::from_millis(100)).await;
            r
        }
        Err(_) => {
            makcu.close().await?;
            Makcu::high_speed()
        }
    }?;

    tracing::info!("{} 고성능 모드로 연결됨", makcu.port_name());
    tracing::info!("버전: {}", makcu.version().await?);
    Ok(makcu)
}
