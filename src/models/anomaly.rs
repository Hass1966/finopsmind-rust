use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Anomaly {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub date: NaiveDate,
    pub actual_amount: rust_decimal::Decimal,
    pub expected_amount: rust_decimal::Decimal,
    pub deviation: rust_decimal::Decimal,
    pub deviation_pct: rust_decimal::Decimal,
    pub score: rust_decimal::Decimal,
    pub severity: String,
    pub status: String,
    pub provider: String,
    pub service: String,
    pub account_id: String,
    pub region: String,
    pub root_cause: Option<String>,
    pub notes: Option<String>,
    pub detected_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub acknowledged_by: Option<Uuid>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AnomalySummary {
    pub total: i64,
    pub open: i64,
    pub acknowledged: i64,
    pub resolved: i64,
    pub critical: i64,
    pub high: i64,
    pub medium: i64,
    pub low: i64,
    pub total_impact: f64,
    pub currency: String,
}

#[derive(Debug, Deserialize)]
pub struct AnomalyQueryParams {
    pub severity: Option<String>,
    pub status: Option<String>,
    pub provider: Option<String>,
    pub service: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAnomalyRequest {
    pub status: Option<String>,
    pub notes: Option<String>,
    pub root_cause: Option<String>,
}

pub fn classify_severity(deviation_pct: f64) -> String {
    if deviation_pct >= 100.0 {
        "critical".into()
    } else if deviation_pct >= 50.0 {
        "high".into()
    } else if deviation_pct >= 25.0 {
        "medium".into()
    } else {
        "low".into()
    }
}
