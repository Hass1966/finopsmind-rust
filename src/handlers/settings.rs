use axum::{extract::State, Extension, Json};

use crate::auth::Claims;
use crate::db::OrgRepo;
use crate::errors::AppError;
use crate::handlers::AppState;

pub async fn get_settings(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    let org = OrgRepo::get_by_id(&state.pool, claims.org_id).await?;

    Ok(Json(serde_json::json!({
        "organization": {
            "id": org.id,
            "name": org.name,
        },
        "settings": org.settings,
    })))
}

pub async fn update_settings(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(settings): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    if claims.role != "admin" {
        return Err(AppError::forbidden("Only admins can update settings"));
    }

    let org = OrgRepo::update_settings(&state.pool, claims.org_id, &settings).await?;

    Ok(Json(serde_json::json!({
        "organization": {
            "id": org.id,
            "name": org.name,
        },
        "settings": org.settings,
    })))
}
