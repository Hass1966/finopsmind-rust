use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Budget {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub amount: rust_decimal::Decimal,
    pub currency: String,
    pub period: String,
    pub filters: serde_json::Value,
    pub thresholds: serde_json::Value,
    pub status: String,
    pub current_spend: rust_decimal::Decimal,
    pub forecasted_spend: rust_decimal::Decimal,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBudgetRequest {
    pub name: String,
    pub amount: f64,
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default = "default_period")]
    pub period: String,
    #[serde(default)]
    pub filters: serde_json::Value,
    #[serde(default)]
    pub thresholds: serde_json::Value,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

fn default_currency() -> String {
    "USD".into()
}
fn default_period() -> String {
    "monthly".into()
}

#[derive(Debug, Deserialize)]
pub struct UpdateBudgetRequest {
    pub name: Option<String>,
    pub amount: Option<f64>,
    pub period: Option<String>,
    pub filters: Option<serde_json::Value>,
    pub thresholds: Option<serde_json::Value>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BudgetSummary {
    pub total_budgets: i64,
    pub active_count: i64,
    pub warning_count: i64,
    pub exceeded_count: i64,
    pub total_allocated: f64,
    pub total_spent: f64,
    pub currency: String,
}
