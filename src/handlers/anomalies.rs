use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use uuid::Uuid;

use crate::auth::Claims;
use crate::db::AnomalyRepo;
use crate::errors::AppError;
use crate::models::{Anomaly, AnomalyQueryParams, AnomalySummary, Pagination, PaginatedResponse, UpdateAnomalyRequest};
use crate::handlers::AppState;

pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<AnomalyQueryParams>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<PaginatedResponse<Anomaly>>, AppError> {
    let page = params.page.unwrap_or(1);
    let page_size = params.page_size.unwrap_or(20);
    let offset = (page - 1) * page_size;

    let (anomalies, total) = AnomalyRepo::list(
        &state.pool,
        claims.org_id,
        params.severity.as_deref(),
        params.status.as_deref(),
        page_size,
        offset,
    )
    .await?;

    Ok(Json(PaginatedResponse {
        data: anomalies,
        pagination: Pagination::new(page, page_size, total),
    }))
}

pub async fn get_summary(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<AnomalySummary>, AppError> {
    let summary = AnomalyRepo::get_summary(&state.pool, claims.org_id).await?;
    Ok(Json(summary))
}

pub async fn update_anomaly(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(update_req): Json<UpdateAnomalyRequest>,
) -> Result<Json<Anomaly>, AppError> {
    let anomaly = AnomalyRepo::update(&state.pool, claims.org_id, id, &update_req).await?;
    Ok(Json(anomaly))
}

pub async fn acknowledge(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Anomaly>, AppError> {
    let anomaly = AnomalyRepo::acknowledge(&state.pool, claims.org_id, id, claims.sub).await?;
    Ok(Json(anomaly))
}

pub async fn resolve(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Anomaly>, AppError> {
    let anomaly = AnomalyRepo::resolve(&state.pool, claims.org_id, id).await?;
    Ok(Json(anomaly))
}
