use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use uuid::Uuid;

use crate::auth::Claims;
use crate::db::RecommendationRepo;
use crate::errors::AppError;
use crate::models::{
    Pagination, PaginatedResponse, Recommendation, RecommendationQueryParams,
    RecommendationSummary, UpdateRecommendationStatusRequest,
};
use crate::handlers::AppState;

pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<RecommendationQueryParams>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<PaginatedResponse<Recommendation>>, AppError> {
    let page = params.page.unwrap_or(1);
    let page_size = params.page_size.unwrap_or(20);
    let offset = (page - 1) * page_size;

    let (recs, total) = RecommendationRepo::list(
        &state.pool,
        claims.org_id,
        params.status.as_deref(),
        params.rec_type.as_deref(),
        params.impact.as_deref(),
        page_size,
        offset,
    )
    .await?;

    Ok(Json(PaginatedResponse {
        data: recs,
        pagination: Pagination::new(page, page_size, total),
    }))
}

pub async fn get_by_id(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Recommendation>, AppError> {
    let rec = RecommendationRepo::get_by_id(&state.pool, claims.org_id, id).await
        .map_err(|_| AppError::not_found("Recommendation", &id.to_string()))?;
    Ok(Json(rec))
}

pub async fn update_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(update_req): Json<UpdateRecommendationStatusRequest>,
) -> Result<Json<Recommendation>, AppError> {
    let rec = RecommendationRepo::update_status(
        &state.pool,
        claims.org_id,
        id,
        &update_req.status,
        update_req.notes.as_deref(),
    )
    .await?;
    Ok(Json(rec))
}

pub async fn dismiss(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Recommendation>, AppError> {
    let rec = RecommendationRepo::update_status(&state.pool, claims.org_id, id, "dismissed", None).await?;
    Ok(Json(rec))
}

pub async fn get_summary(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<RecommendationSummary>, AppError> {
    let summary = RecommendationRepo::get_summary(&state.pool, claims.org_id).await?;
    Ok(Json(summary))
}

pub async fn generate(
    State(_state): State<AppState>,
    Extension(_claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    // In a full implementation this would run the recommendation rules engine.
    // For now, return a message indicating the job has been triggered.
    Ok(Json(serde_json::json!({
        "message": "Recommendation generation triggered",
        "status": "running"
    })))
}

pub async fn get_terraform(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    let rec = RecommendationRepo::get_by_id(&state.pool, claims.org_id, id).await
        .map_err(|_| AppError::not_found("Recommendation", &id.to_string()))?;

    let terraform = rec.terraform_code.unwrap_or_else(|| {
        format!(
            r#"# Terraform for recommendation: {}
# Type: {}
# Resource: {}
# Estimated savings: ${}/month

resource "aws_instance" "optimized" {{
  # Apply recommended configuration
  instance_type = "{}"
}}
"#,
            rec.id,
            rec.rec_type,
            rec.resource_id,
            rec.estimated_savings,
            rec.recommended_config.get("instance_type").and_then(|v| v.as_str()).unwrap_or("t3.medium"),
        )
    });

    Ok(Json(serde_json::json!({
        "recommendation_id": rec.id,
        "terraform": terraform,
    })))
}
