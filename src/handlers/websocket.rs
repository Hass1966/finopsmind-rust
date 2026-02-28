use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::Response,
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;

use crate::handlers::AppState;

#[derive(Debug, Deserialize)]
pub struct WsParams {
    pub token: String,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsParams>,
    State(state): State<AppState>,
) -> Response {
    // Validate JWT from query param
    match state.jwt.validate_token(&params.token) {
        Ok(claims) => {
            let hub = state.ws_hub.clone();
            ws.on_upgrade(move |socket| handle_socket(socket, claims.org_id, hub))
        }
        Err(_) => {
            ws.on_upgrade(|mut socket| async move {
                let _ = socket.send(Message::Close(Some(axum::extract::ws::CloseFrame {
                    code: 4001,
                    reason: "Unauthorized".into(),
                }))).await;
            })
        }
    }
}

async fn handle_socket(socket: WebSocket, org_id: uuid::Uuid, hub: crate::ws::WsHub) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to org-specific broadcast channel
    let mut rx = hub.subscribe(org_id).await;

    // Send welcome message
    let welcome = serde_json::json!({
        "type": "connected",
        "org_id": org_id.to_string(),
        "message": "WebSocket connection established"
    });
    let _ = sender.send(Message::Text(serde_json::to_string(&welcome).unwrap().into())).await;

    // Spawn task to forward broadcast messages to this client
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Spawn task to handle incoming messages
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = &mut send_task => {
            recv_task.abort();
        }
        _ = &mut recv_task => {
            send_task.abort();
        }
    }

    tracing::info!(org_id = %org_id, "WebSocket client disconnected");
}
