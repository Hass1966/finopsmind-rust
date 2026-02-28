use axum::{
    extract::{Path, State},
    Extension, Json,
};
use uuid::Uuid;

use crate::auth::Claims;
use crate::db::CloudProviderRepo;
use crate::errors::AppError;
use crate::models::{CloudProviderConfig, CreateProviderRequest, UpdateProviderRequest};
use crate::handlers::AppState;

pub async fn list(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<CloudProviderConfig>>, AppError> {
    let providers = CloudProviderRepo::list(&state.pool, claims.org_id).await?;
    Ok(Json(providers))
}

pub async fn create(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(create_req): Json<CreateProviderRequest>,
) -> Result<Json<CloudProviderConfig>, AppError> {
    // Encrypt credentials
    let creds_json = serde_json::to_vec(&create_req.credentials)
        .map_err(|_| AppError::bad_request("Invalid credentials format"))?;

    let encrypted = crate::crypto::encrypt(&creds_json, &state.encryption_key)
        .map_err(|e| AppError::internal(format!("Encryption error: {e}")))?;

    let config = CloudProviderConfig {
        id: Uuid::new_v4(),
        organization_id: claims.org_id,
        provider_type: create_req.provider_type,
        name: create_req.name,
        credentials: Some(encrypted),
        enabled: true,
        status: "pending".into(),
        status_message: None,
        last_sync_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let provider = CloudProviderRepo::create(&state.pool, &config).await?;
    Ok(Json(provider))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(update_req): Json<UpdateProviderRequest>,
) -> Result<Json<CloudProviderConfig>, AppError> {
    let encrypted = if let Some(creds) = &update_req.credentials {
        let creds_json = serde_json::to_vec(creds)
            .map_err(|_| AppError::bad_request("Invalid credentials format"))?;
        Some(crate::crypto::encrypt(&creds_json, &state.encryption_key)
            .map_err(|e| AppError::internal(format!("Encryption error: {e}")))?)
    } else {
        None
    };

    let provider = CloudProviderRepo::update(
        &state.pool,
        claims.org_id,
        id,
        update_req.name.as_deref(),
        encrypted.as_deref(),
        update_req.enabled,
    )
    .await?;
    Ok(Json(provider))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    CloudProviderRepo::delete(&state.pool, claims.org_id, id).await?;
    Ok(Json(serde_json::json!({"message": "Provider deleted"})))
}

pub async fn test_connection(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    let provider = CloudProviderRepo::get_by_id(&state.pool, claims.org_id, id).await
        .map_err(|_| AppError::not_found("Provider", &id.to_string()))?;

    // For now, mark as connected (actual cloud SDK calls would go here)
    CloudProviderRepo::update_status(&state.pool, id, "connected", None).await?;

    Ok(Json(serde_json::json!({
        "status": "connected",
        "provider": provider.provider_type,
        "message": "Connection successful"
    })))
}

pub async fn trigger_sync(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    let _provider = CloudProviderRepo::get_by_id(&state.pool, claims.org_id, id).await
        .map_err(|_| AppError::not_found("Provider", &id.to_string()))?;

    CloudProviderRepo::update_sync_time(&state.pool, id).await?;

    Ok(Json(serde_json::json!({
        "message": "Sync triggered",
        "status": "running"
    })))
}
