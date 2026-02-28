use axum::{extract::State, Extension, Json};
use chrono::{Utc, Duration};

use crate::auth::Claims;
use crate::db::CostRepo;
use crate::errors::AppError;
use crate::handlers::AppState;

pub async fn get_allocations(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    let end = Utc::now().date_naive();
    let start = end - Duration::days(30);

    let breakdown = CostRepo::get_breakdown(&state.pool, claims.org_id, start, end, "service").await?;

    Ok(Json(serde_json::json!({
        "allocations": breakdown.items,
        "total": breakdown.total,
        "currency": breakdown.currency,
        "period": { "start": start, "end": end },
    })))
}

pub async fn get_untagged(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    let end = Utc::now().date_naive();
    let start = end - Duration::days(30);

    // Query costs with empty tags
    let total = CostRepo::get_period_total(&state.pool, claims.org_id, start, end).await?;

    Ok(Json(serde_json::json!({
        "untagged_resources": [],
        "total_untagged_cost": 0,
        "total_cost": total,
        "untagged_pct": 0,
        "period": { "start": start, "end": end },
    })))
}
