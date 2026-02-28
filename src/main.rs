#![allow(dead_code)]

mod config;
mod errors;
mod models;
mod db;
mod ml;
mod auth;
mod handlers;
mod jobs;
mod ws;
mod crypto;

use std::sync::Arc;
use axum::{
    middleware,
    routing::{delete, get, post, put, patch},
    Router,
};
use sqlx::postgres::PgPoolOptions;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::auth::{middleware::auth_middleware, middleware::AuthState, jwt::JwtManager};
use crate::config::AppConfig;
use crate::handlers::AppState;
use crate::ws::WsHub;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "finopsmind=info,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    // Load configuration
    let config = AppConfig::load()?;
    tracing::info!("Configuration loaded");

    // Connect to PostgreSQL
    let pool = PgPoolOptions::new()
        .max_connections(config.database.max_connections)
        .connect(&config.database.url())
        .await?;
    tracing::info!("Connected to PostgreSQL");

    // Run migrations
    sqlx::raw_sql(include_str!("../migrations/001_initial_schema.sql"))
        .execute(&pool)
        .await?;
    tracing::info!("Database migrations applied");

    // Initialize JWT manager
    let jwt = Arc::new(JwtManager::new(&config.auth.jwt_secret, config.auth.token_expiry_hours));

    // Initialize WebSocket hub
    let ws_hub = WsHub::new();

    // Create shared state
    let state = AppState {
        pool: pool.clone(),
        jwt: jwt.clone(),
        ws_hub: ws_hub.clone(),
        llm_config: config.llm.clone(),
        encryption_key: config.encryption_key.clone(),
    };

    let auth_state = AuthState {
        jwt: jwt.clone(),
        pool: pool.clone(),
    };

    // Spawn background jobs
    jobs::spawn_background_jobs(pool.clone(), config.jobs.clone(), ws_hub.clone());

    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Public routes (no auth)
    let public_routes = Router::new()
        .route("/health", get(handlers::health::health_check))
        .route("/api/v1/auth/login", post(handlers::auth::login))
        .route("/api/v1/auth/signup", post(handlers::auth::signup));

    // Protected routes (require auth)
    let protected_routes = Router::new()
        // Auth
        .route("/api/v1/auth/me", get(handlers::auth::me))
        .route("/api/v1/auth/api-keys", post(handlers::auth::create_api_key))
        // Costs
        .route("/api/v1/costs/summary", get(handlers::costs::get_summary))
        .route("/api/v1/costs/trend", get(handlers::costs::get_trend))
        .route("/api/v1/costs/breakdown", get(handlers::costs::get_breakdown))
        .route("/api/v1/costs/export", get(handlers::costs::export_csv))
        // Budgets
        .route("/api/v1/budgets", get(handlers::budgets::list).post(handlers::budgets::create))
        .route("/api/v1/budgets/:id", get(handlers::budgets::get_by_id).put(handlers::budgets::update).delete(handlers::budgets::delete))
        // Anomalies
        .route("/api/v1/anomalies", get(handlers::anomalies::list))
        .route("/api/v1/anomalies/summary", get(handlers::anomalies::get_summary))
        .route("/api/v1/anomalies/:id", patch(handlers::anomalies::update_anomaly))
        .route("/api/v1/anomalies/:id/acknowledge", post(handlers::anomalies::acknowledge))
        .route("/api/v1/anomalies/:id/resolve", post(handlers::anomalies::resolve))
        // Recommendations
        .route("/api/v1/recommendations", get(handlers::recommendations::list))
        .route("/api/v1/recommendations/generate", post(handlers::recommendations::generate))
        .route("/api/v1/recommendations/summary", get(handlers::recommendations::get_summary))
        .route("/api/v1/recommendations/:id", get(handlers::recommendations::get_by_id))
        .route("/api/v1/recommendations/:id/status", put(handlers::recommendations::update_status))
        .route("/api/v1/recommendations/:id/dismiss", post(handlers::recommendations::dismiss))
        .route("/api/v1/recommendations/:id/terraform", get(handlers::recommendations::get_terraform))
        // Forecasts
        .route("/api/v1/forecasts", get(handlers::forecasts::list))
        .route("/api/v1/forecasts/latest", get(handlers::forecasts::get_latest))
        // Cloud Providers
        .route("/api/v1/providers", get(handlers::cloud_providers::list).post(handlers::cloud_providers::create))
        .route("/api/v1/providers/:id", put(handlers::cloud_providers::update).delete(handlers::cloud_providers::delete))
        .route("/api/v1/providers/:id/test", post(handlers::cloud_providers::test_connection))
        .route("/api/v1/providers/:id/sync", post(handlers::cloud_providers::trigger_sync))
        // Remediations
        .route("/api/v1/remediations", get(handlers::remediations::list).post(handlers::remediations::propose))
        .route("/api/v1/remediations/summary", get(handlers::remediations::get_summary))
        .route("/api/v1/remediations/:id", get(handlers::remediations::get_by_id))
        .route("/api/v1/remediations/:id/approve", post(handlers::remediations::approve))
        .route("/api/v1/remediations/:id/reject", post(handlers::remediations::reject))
        .route("/api/v1/remediations/:id/cancel", post(handlers::remediations::cancel))
        .route("/api/v1/remediations/:id/rollback", post(handlers::remediations::rollback))
        .route("/api/v1/remediations/rules", get(handlers::remediations::list_rules).post(handlers::remediations::create_rule))
        .route("/api/v1/remediations/rules/:id", put(handlers::remediations::update_rule).delete(handlers::remediations::delete_rule))
        // Policies
        .route("/api/v1/policies", get(handlers::policies::list).post(handlers::policies::create))
        .route("/api/v1/policies/summary", get(handlers::policies::get_summary))
        .route("/api/v1/policies/violations", get(handlers::policies::get_violations))
        .route("/api/v1/policies/:id", get(handlers::policies::get_by_id))
        // Reports
        .route("/api/v1/reports/executive-summary", get(handlers::reports::executive_summary))
        .route("/api/v1/reports/comparison", get(handlers::reports::cost_comparison))
        .route("/api/v1/reports/export/csv", get(handlers::reports::export_csv))
        .route("/api/v1/reports/export/json", get(handlers::reports::export_json))
        // Chat
        .route("/api/v1/chat", post(handlers::chat::chat))
        // Settings
        .route("/api/v1/settings", get(handlers::settings::get_settings).put(handlers::settings::update_settings))
        // Allocations
        .route("/api/v1/allocations", get(handlers::allocations::get_allocations))
        .route("/api/v1/allocations/untagged", get(handlers::allocations::get_untagged))
        // Apply auth middleware
        .layer(middleware::from_fn_with_state(auth_state, auth_middleware));

    // WebSocket route (auth via query param)
    let ws_routes = Router::new()
        .route("/ws", get(handlers::websocket::ws_handler));

    // Combine all routes
    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .merge(ws_routes)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    tracing::info!("Starting FinOpsMind server on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Server shut down gracefully");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received");
}
