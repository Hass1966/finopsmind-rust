use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForecastPoint {
    pub date: NaiveDate,
    pub predicted: f64,
    pub lower_bound: f64,
    pub upper_bound: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Forecast {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub generated_at: DateTime<Utc>,
    pub model_version: String,
    pub granularity: String,
    pub predictions: serde_json::Value,
    pub total_forecasted: rust_decimal::Decimal,
    pub confidence_level: rust_decimal::Decimal,
    pub currency: String,
    pub service_filter: Option<String>,
    pub account_filter: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ForecastSummary {
    pub current_month_forecast: f64,
    pub next_month_forecast: f64,
    pub quarter_forecast: f64,
    pub trend_direction: String,
    pub change_percent: f64,
    pub currency: String,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ForecastQueryParams {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub service: Option<String>,
    pub account_id: Option<String>,
}
