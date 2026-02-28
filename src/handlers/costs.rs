use axum::{extract::{Query, State}, Extension, Json};
use chrono::{Utc, Duration};

use crate::auth::Claims;
use crate::db::CostRepo;
use crate::errors::AppError;
use crate::models::{CostBreakdown, CostQueryParams, CostSummary, CostTrend};
use crate::handlers::AppState;

pub async fn get_summary(
    State(state): State<AppState>,
    Query(params): Query<CostQueryParams>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<CostSummary>, AppError> {
    let end = params.end_date.unwrap_or_else(|| Utc::now().date_naive());
    let start = params.start_date.unwrap_or_else(|| end - Duration::days(30));

    let summary = CostRepo::get_summary(&state.pool, claims.org_id, start, end).await?;
    Ok(Json(summary))
}

pub async fn get_trend(
    State(state): State<AppState>,
    Query(params): Query<CostQueryParams>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<CostTrend>, AppError> {
    let end = params.end_date.unwrap_or_else(|| Utc::now().date_naive());
    let start = params.start_date.unwrap_or_else(|| end - Duration::days(30));
    let granularity = params.granularity.as_deref().unwrap_or("daily");

    let trend = CostRepo::get_trend(&state.pool, claims.org_id, start, end, granularity).await?;
    Ok(Json(trend))
}

pub async fn get_breakdown(
    State(state): State<AppState>,
    Query(params): Query<CostQueryParams>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<CostBreakdown>, AppError> {
    let end = params.end_date.unwrap_or_else(|| Utc::now().date_naive());
    let start = params.start_date.unwrap_or_else(|| end - Duration::days(30));
    let dimension = params.dimension.as_deref().unwrap_or("service");

    let breakdown = CostRepo::get_breakdown(&state.pool, claims.org_id, start, end, dimension).await?;
    Ok(Json(breakdown))
}

pub async fn export_csv(
    State(state): State<AppState>,
    Query(params): Query<CostQueryParams>,
    Extension(claims): Extension<Claims>,
) -> Result<axum::response::Response, AppError> {
    let end = params.end_date.unwrap_or_else(|| Utc::now().date_naive());
    let start = params.start_date.unwrap_or_else(|| end - Duration::days(30));

    let records = CostRepo::export_csv(&state.pool, claims.org_id, start, end).await?;

    let mut csv = String::from("date,amount,currency,provider,service,account_id,region,resource_id\n");
    for r in &records {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            r.date, r.amount, r.currency, r.provider, r.service, r.account_id, r.region, r.resource_id
        ));
    }

    Ok(axum::response::Response::builder()
        .header("Content-Type", "text/csv")
        .header("Content-Disposition", "attachment; filename=costs.csv")
        .body(axum::body::Body::from(csv))
        .unwrap())
}
