use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CostRecord {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub date: NaiveDate,
    pub amount: rust_decimal::Decimal,
    pub currency: String,
    pub provider: String,
    pub service: String,
    pub account_id: String,
    pub region: String,
    pub resource_id: String,
    pub tags: serde_json::Value,
    pub estimated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummary {
    pub total_cost: f64,
    pub currency: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub by_service: Vec<CostBreakdownItem>,
    pub previous_period_cost: Option<f64>,
    pub change_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdownItem {
    pub name: String,
    pub amount: f64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdown {
    pub dimension: String,
    pub items: Vec<CostBreakdownItem>,
    pub total: f64,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostTrend {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub granularity: String,
    pub data_points: Vec<CostDataPoint>,
    pub total_cost: f64,
    pub avg_daily_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostDataPoint {
    pub date: NaiveDate,
    pub amount: f64,
    pub provider: Option<String>,
    pub service: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CostQueryParams {
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub provider: Option<String>,
    pub service: Option<String>,
    pub account_id: Option<String>,
    pub region: Option<String>,
    pub granularity: Option<String>,
    pub dimension: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
