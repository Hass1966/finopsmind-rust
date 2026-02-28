use axum::{extract::{Query, State}, Extension, Json};
use chrono::{Utc, Duration};
use serde::Deserialize;

use crate::auth::Claims;
use crate::db::{AnomalyRepo, BudgetRepo, CostRepo, RecommendationRepo};
use crate::errors::AppError;
use crate::handlers::AppState;

#[derive(Debug, Deserialize)]
pub struct ReportParams {
    pub period: Option<String>,
    pub start_date: Option<chrono::NaiveDate>,
    pub end_date: Option<chrono::NaiveDate>,
}

pub async fn executive_summary(
    State(state): State<AppState>,
    Query(params): Query<ReportParams>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    let end = params.end_date.unwrap_or_else(|| Utc::now().date_naive());
    let start = params.start_date.unwrap_or_else(|| end - Duration::days(30));

    let cost_summary = CostRepo::get_summary(&state.pool, claims.org_id, start, end).await?;
    let anomaly_summary = AnomalyRepo::get_summary(&state.pool, claims.org_id).await?;
    let rec_summary = RecommendationRepo::get_summary(&state.pool, claims.org_id).await?;
    let budget_summary = BudgetRepo::get_summary(&state.pool, claims.org_id).await?;

    Ok(Json(serde_json::json!({
        "period": { "start": start, "end": end },
        "costs": cost_summary,
        "anomalies": anomaly_summary,
        "recommendations": rec_summary,
        "budgets": budget_summary,
    })))
}

pub async fn cost_comparison(
    State(state): State<AppState>,
    Query(params): Query<ReportParams>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    let end = params.end_date.unwrap_or_else(|| Utc::now().date_naive());
    let start = params.start_date.unwrap_or_else(|| end - Duration::days(30));
    let days = (end - start).num_days();

    let current = CostRepo::get_summary(&state.pool, claims.org_id, start, end).await?;

    let prev_end = start - Duration::days(1);
    let prev_start = prev_end - Duration::days(days);
    let previous = CostRepo::get_summary(&state.pool, claims.org_id, prev_start, prev_end).await?;

    let change = if previous.total_cost > 0.0 {
        ((current.total_cost - previous.total_cost) / previous.total_cost) * 100.0
    } else {
        0.0
    };

    Ok(Json(serde_json::json!({
        "current_period": { "start": start, "end": end, "total": current.total_cost },
        "previous_period": { "start": prev_start, "end": prev_end, "total": previous.total_cost },
        "change_pct": change,
        "current_breakdown": current.by_service,
        "previous_breakdown": previous.by_service,
    })))
}

pub async fn export_csv(
    State(state): State<AppState>,
    Query(params): Query<ReportParams>,
    Extension(claims): Extension<Claims>,
) -> Result<axum::response::Response, AppError> {
    let end = params.end_date.unwrap_or_else(|| Utc::now().date_naive());
    let start = params.start_date.unwrap_or_else(|| end - Duration::days(30));

    let records = CostRepo::export_csv(&state.pool, claims.org_id, start, end).await?;

    let mut csv = String::from("date,amount,currency,provider,service,account_id,region\n");
    for r in &records {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            r.date, r.amount, r.currency, r.provider, r.service, r.account_id, r.region
        ));
    }

    Ok(axum::response::Response::builder()
        .header("Content-Type", "text/csv")
        .header("Content-Disposition", "attachment; filename=report.csv")
        .body(axum::body::Body::from(csv))
        .unwrap())
}

pub async fn export_json(
    State(state): State<AppState>,
    Query(params): Query<ReportParams>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    let end = params.end_date.unwrap_or_else(|| Utc::now().date_naive());
    let start = params.start_date.unwrap_or_else(|| end - Duration::days(30));

    let cost_summary = CostRepo::get_summary(&state.pool, claims.org_id, start, end).await?;
    let trend = CostRepo::get_trend(&state.pool, claims.org_id, start, end, "daily").await?;

    Ok(Json(serde_json::json!({
        "report_type": "cost_report",
        "period": { "start": start, "end": end },
        "summary": cost_summary,
        "trend": trend,
    })))
}
