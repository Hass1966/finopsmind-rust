use axum::{
    extract::{Path, State},
    Extension, Json,
};
use uuid::Uuid;

use crate::auth::Claims;
use crate::db::BudgetRepo;
use crate::errors::AppError;
use crate::models::{Budget, BudgetSummary, CreateBudgetRequest, UpdateBudgetRequest};
use crate::handlers::AppState;

pub async fn list(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<Budget>>, AppError> {
    let budgets = BudgetRepo::list(&state.pool, claims.org_id).await?;
    Ok(Json(budgets))
}

pub async fn create(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(create_req): Json<CreateBudgetRequest>,
) -> Result<Json<Budget>, AppError> {
    let budget = BudgetRepo::create(&state.pool, claims.org_id, &create_req).await?;
    Ok(Json(budget))
}

pub async fn get_by_id(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Budget>, AppError> {
    let budget = BudgetRepo::get_by_id(&state.pool, claims.org_id, id).await
        .map_err(|_| AppError::not_found("Budget", &id.to_string()))?;
    Ok(Json(budget))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(update_req): Json<UpdateBudgetRequest>,
) -> Result<Json<Budget>, AppError> {
    let budget = BudgetRepo::update(&state.pool, claims.org_id, id, &update_req).await?;
    Ok(Json(budget))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    BudgetRepo::delete(&state.pool, claims.org_id, id).await?;
    Ok(Json(serde_json::json!({"message": "Budget deleted"})))
}
