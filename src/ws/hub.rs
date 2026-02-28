use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct WsMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub org_id: Uuid,
    pub data: serde_json::Value,
}

/// WebSocket hub for broadcasting real-time updates per organization.
#[derive(Clone)]
pub struct WsHub {
    /// Per-org broadcast channels
    channels: Arc<RwLock<HashMap<Uuid, broadcast::Sender<String>>>>,
}

impl WsHub {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create a broadcast channel for an organization.
    pub async fn subscribe(&self, org_id: Uuid) -> broadcast::Receiver<String> {
        let mut channels = self.channels.write().await;
        let sender = channels.entry(org_id).or_insert_with(|| {
            let (tx, _) = broadcast::channel(256);
            tx
        });
        sender.subscribe()
    }

    /// Broadcast a message to all connected clients of an organization.
    pub async fn broadcast(&self, msg: WsMessage) {
        let channels = self.channels.read().await;
        if let Some(sender) = channels.get(&msg.org_id) {
            if let Ok(json) = serde_json::to_string(&msg) {
                let _ = sender.send(json);
            }
        }
    }

    /// Send a cost update event.
    pub async fn send_cost_update(&self, org_id: Uuid, data: serde_json::Value) {
        self.broadcast(WsMessage {
            msg_type: "cost_update".into(),
            org_id,
            data,
        })
        .await;
    }

    /// Send an anomaly alert event.
    pub async fn send_anomaly_alert(&self, org_id: Uuid, data: serde_json::Value) {
        self.broadcast(WsMessage {
            msg_type: "anomaly_alert".into(),
            org_id,
            data,
        })
        .await;
    }

    /// Send a budget alert event.
    pub async fn send_budget_alert(&self, org_id: Uuid, data: serde_json::Value) {
        self.broadcast(WsMessage {
            msg_type: "budget_alert".into(),
            org_id,
            data,
        })
        .await;
    }

    /// Send a recommendation event.
    pub async fn send_recommendation(&self, org_id: Uuid, data: serde_json::Value) {
        self.broadcast(WsMessage {
            msg_type: "recommendation".into(),
            org_id,
            data,
        })
        .await;
    }

    /// Send a remediation status update.
    pub async fn send_remediation_update(&self, org_id: Uuid, data: serde_json::Value) {
        self.broadcast(WsMessage {
            msg_type: "remediation_update".into(),
            org_id,
            data,
        })
        .await;
    }

    /// Send a policy violation event.
    pub async fn send_policy_violation(&self, org_id: Uuid, data: serde_json::Value) {
        self.broadcast(WsMessage {
            msg_type: "policy_violation".into(),
            org_id,
            data,
        })
        .await;
    }
}
