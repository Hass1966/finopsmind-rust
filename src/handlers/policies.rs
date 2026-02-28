use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use uuid::Uuid;

use crate::auth::Claims;
use crate::db::PolicyRepo;
use crate::errors::AppError;
use crate::models::*;
use crate::handlers::AppState;

pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<PolicyQueryParams>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<PaginatedResponse<Policy>>, AppError> {
    let page = params.page.unwrap_or(1);
    let page_size = params.page_size.unwrap_or(20);
    let offset = (page - 1) * page_size;

    let (policies, total) = PolicyRepo::list(&state.pool, claims.org_id, page_size, offset).await?;

    Ok(Json(PaginatedResponse {
        data: policies,
        pagination: Pagination::new(page, page_size, total),
    }))
}

pub async fn get_by_id(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Policy>, AppError> {
    let policy = PolicyRepo::get_by_id(&state.pool, claims.org_id, id).await
        .map_err(|_| AppError::not_found("Policy", &id.to_string()))?;
    Ok(Json(policy))
}

pub async fn create(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(create_req): Json<CreatePolicyRequest>,
) -> Result<Json<Policy>, AppError> {
    let policy = Policy {
        id: Uuid::new_v4(),
        organization_id: claims.org_id,
        name: create_req.name,
        description: create_req.description,
        policy_type: create_req.policy_type,
        enforcement_mode: create_req.enforcement_mode,
        enabled: create_req.enabled,
        conditions: create_req.conditions,
        providers: create_req.providers,
        environments: create_req.environments,
        created_by: Some(claims.sub),
        last_evaluated_at: None,
        violation_count: 0,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let created = PolicyRepo::create(&state.pool, &policy).await?;
    Ok(Json(created))
}

pub async fn get_violations(
    State(state): State<AppState>,
    Query(params): Query<ViolationQueryParams>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<PaginatedResponse<PolicyViolation>>, AppError> {
    let page = params.page.unwrap_or(1);
    let page_size = params.page_size.unwrap_or(20);
    let offset = (page - 1) * page_size;

    let (violations, total) = PolicyRepo::list_violations(
        &state.pool,
        claims.org_id,
        params.policy_id,
        params.status.as_deref(),
        params.severity.as_deref(),
        page_size,
        offset,
    )
    .await?;

    Ok(Json(PaginatedResponse {
        data: violations,
        pagination: Pagination::new(page, page_size, total),
    }))
}

pub async fn get_summary(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<PolicySummary>, AppError> {
    let summary = PolicyRepo::get_summary(&state.pool, claims.org_id).await?;
    Ok(Json(summary))
}
