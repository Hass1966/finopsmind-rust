use axum::{extract::{Query, State}, Extension, Json};
use chrono::{Utc, Duration};

use crate::auth::Claims;
use crate::cache;
use crate::db::CostRepo;
use crate::errors::AppError;
use crate::models::{AiCostSummary, CostBreakdown, CostQueryParams, CostSummary, CostTrend};
use crate::handlers::AppState;

pub async fn get_summary(
    State(state): State<AppState>,
    Query(params): Query<CostQueryParams>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<CostSummary>, AppError> {
    let end = params.end_date.unwrap_or_else(|| Utc::now().date_naive());
    let start = params.start_date.unwrap_or_else(|| end - Duration::days(30));

    let cache_key = format!("costs:{}:summary:{}:{}", claims.org_id, start, end);
    let pool = state.pool.clone();

    let summary = cache::get_or_set(&state.redis, &cache_key, 300, || async {
        CostRepo::get_summary(&pool, claims.org_id, start, end)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    })
    .await
    .map_err(|e| AppError::internal(format!("Cache/query error: {e}")))?;

    Ok(Json(summary))
}

pub async fn get_trend(
    State(state): State<AppState>,
    Query(params): Query<CostQueryParams>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<CostTrend>, AppError> {
    let end = params.end_date.unwrap_or_else(|| Utc::now().date_naive());
    let start = params.start_date.unwrap_or_else(|| end - Duration::days(30));
    let granularity = params.granularity.clone().unwrap_or_else(|| "daily".into());

    let cache_key = format!("costs:{}:trend:{}:{}:{}", claims.org_id, start, end, granularity);
    let pool = state.pool.clone();
    let gran = granularity.clone();

    let trend = cache::get_or_set(&state.redis, &cache_key, 300, || async {
        CostRepo::get_trend(&pool, claims.org_id, start, end, &gran)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    })
    .await
    .map_err(|e| AppError::internal(format!("Cache/query error: {e}")))?;

    Ok(Json(trend))
}

pub async fn get_breakdown(
    State(state): State<AppState>,
    Query(params): Query<CostQueryParams>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<CostBreakdown>, AppError> {
    let end = params.end_date.unwrap_or_else(|| Utc::now().date_naive());
    let start = params.start_date.unwrap_or_else(|| end - Duration::days(30));
    let dimension = params.dimension.clone().unwrap_or_else(|| "service".into());

    let cache_key = format!("costs:{}:breakdown:{}:{}:{}", claims.org_id, start, end, dimension);
    let pool = state.pool.clone();
    let dim = dimension.clone();

    let breakdown = cache::get_or_set(&state.redis, &cache_key, 300, || async {
        CostRepo::get_breakdown(&pool, claims.org_id, start, end, &dim)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    })
    .await
    .map_err(|e| AppError::internal(format!("Cache/query error: {e}")))?;

    Ok(Json(breakdown))
}

pub async fn get_ai_costs(
    State(state): State<AppState>,
    Query(params): Query<CostQueryParams>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<AiCostSummary>, AppError> {
    let end = params.end_date.unwrap_or_else(|| Utc::now().date_naive());
    let start = params.start_date.unwrap_or_else(|| end - Duration::days(30));

    let cache_key = format!("costs:{}:ai:{}:{}", claims.org_id, start, end);
    let pool = state.pool.clone();

    let summary = cache::get_or_set(&state.redis, &cache_key, 300, || async {
        CostRepo::get_ai_summary(&pool, claims.org_id, start, end)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    })
    .await
    .map_err(|e| AppError::internal(format!("Cache/query error: {e}")))?;

    Ok(Json(summary))
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
