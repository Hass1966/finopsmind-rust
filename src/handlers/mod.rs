pub mod auth;
pub mod costs;
pub mod budgets;
pub mod anomalies;
pub mod forecasts;
pub mod recommendations;
pub mod remediations;
pub mod cloud_providers;
pub mod policies;
pub mod reports;
pub mod chat;
pub mod websocket;
pub mod health;
pub mod settings;
pub mod allocations;

use std::sync::Arc;
use crate::auth::jwt::JwtManager;
use crate::config::LlmConfig;
use crate::ws::WsHub;

/// Shared application state available to all handlers.
#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub jwt: Arc<JwtManager>,
    pub ws_hub: WsHub,
    pub llm_config: LlmConfig,
    pub encryption_key: String,
}
