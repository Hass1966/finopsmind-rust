use axum::{extract::{Query, State}, Extension, Json};

use crate::auth::Claims;
use crate::db::ForecastRepo;
use crate::errors::AppError;
use crate::models::{Forecast, ForecastQueryParams, Pagination, PaginatedResponse};
use crate::handlers::AppState;

pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<ForecastQueryParams>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<PaginatedResponse<Forecast>>, AppError> {
    let page = params.page.unwrap_or(1);
    let page_size = params.page_size.unwrap_or(20);
    let offset = (page - 1) * page_size;

    let (forecasts, total) = ForecastRepo::list(&state.pool, claims.org_id, page_size, offset).await?;

    Ok(Json(PaginatedResponse {
        data: forecasts,
        pagination: Pagination::new(page, page_size, total),
    }))
}

pub async fn get_latest(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    match ForecastRepo::get_latest(&state.pool, claims.org_id).await? {
        Some(forecast) => Ok(Json(serde_json::to_value(forecast).unwrap())),
        None => Ok(Json(serde_json::json!({"message": "No forecasts available"}))),
    }
}
