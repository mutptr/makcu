pub type Makcu = makcu::Makcu<makcu::HighSpeed>;

use anyhow::Context;
use tokio::time::sleep;

use crate::server::Server;

mod crypto;
mod handler;
mod protocol;
mod server;

const BIND_ADDRESS: &str = "0.0.0.0:39026";

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
        let str = std::iter::successors(err.source(), |err| err.source())
            .fold(err.to_string(), |acc, err| format!("{acc}: {err}"));
        tracing::error!("{str}");
    }
}

async fn start() -> anyhow::Result<()> {
    let makcu = connect_makcu().await?;

    let server = Server::new(BIND_ADDRESS, makcu).await?;
    server.run().await
}

async fn connect_makcu() -> anyhow::Result<Makcu> {
    let makcu = makcu::Makcu::normal().context("Makcu 연결 실패")?;
    tracing::info!("{} 연결됨", makcu.port_name());

    let version_result = makcu.version().await;
    let makcu = match version_result {
        Ok(_) => {
            tracing::info!("고성능 모드 활성화");
            let r = makcu.enable_high_speed_mode().await;
            sleep(std::time::Duration::from_millis(100)).await;
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
