use std::sync::Arc;

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use serde::Deserialize;
use tokio::sync::watch;

use crate::{Makcu, emulator::InputEmulator};

#[derive(Clone)]
pub struct AppState {
    key_state: watch::Receiver<u8>,
    emulator: Arc<InputEmulator>,
}

impl AppState {
    pub fn new(makcu: Makcu) -> Self {
        let key_state = makcu.subscribe_buttons();
        let emulator = InputEmulator::new(makcu);

        Self {
            key_state,
            emulator,
        }
    }
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (sender, receiver) = socket.split();

    tracing::info!("클라이언트 연결됨");

    let sender_task = handle_sender(sender, state.key_state.clone());
    let receiver_task = handle_receiver(receiver, state.clone());

    _ = tokio::try_join!(sender_task, receiver_task);

    tracing::info!("클라이언트 연결 종료");

    _ = state.emulator.unlock().await;
}

async fn handle_sender(
    mut sender: SplitSink<WebSocket, Message>,
    mut key_state: watch::Receiver<u8>,
) -> anyhow::Result<()> {
    loop {
        key_state.changed().await?;
        let key = *key_state.borrow();
        tracing::debug!("웹소켓 키 전달: {key}");
        sender.send(Message::Binary(vec![key].into())).await?;
    }
}

async fn handle_receiver(
    mut receiver: SplitStream<WebSocket>,
    state: AppState,
) -> anyhow::Result<()> {
    loop {
        match receiver.next().await {
            Some(Ok(Message::Binary(bytes))) => {
                handle_message(&bytes, &state).await?;
            }
            Some(Ok(Message::Close(_))) => break,
            Some(Err(_)) => break,
            None => break,
            _ => continue,
        }
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum Command {
    #[serde(rename = "0")]
    MouseMove { a: i32, b: i32 },

    #[serde(rename = "1")]
    Lock,

    #[serde(rename = "2")]
    Unlock,

    #[serde(rename = "3")]
    Pending { b: bool },

    #[serde(rename = "4")]
    Click,
}

async fn handle_message(text: &[u8], state: &AppState) -> anyhow::Result<()> {
    if let Ok(command) = rmp_serde::from_slice::<Command>(text) {
        match command {
            Command::MouseMove { a, b } => {
                tracing::debug!(x = a, y = b, "마우스 이동");
                state.emulator.mouse_move(a, b).await?;
            }
            Command::Lock => {
                tracing::debug!("lock");
                state.emulator.lock().await?;
            }
            Command::Unlock => {
                tracing::debug!("unlock");
                state.emulator.unlock().await?;
            }
            Command::Pending { b } => {
                state.emulator.pending(b).await?;
            }
            Command::Click => {
                tracing::debug!("클릭");
                state.emulator.click().await?;
            }
        }
    }

    Ok(())
}
