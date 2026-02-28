use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Recommendation {
    pub id: Uuid,
    pub organization_id: Uuid,
    #[sqlx(rename = "type")]
    pub rec_type: String,
    pub provider: String,
    pub account_id: String,
    pub region: String,
    pub resource_id: String,
    pub resource_type: String,
    pub current_config: serde_json::Value,
    pub recommended_config: serde_json::Value,
    pub estimated_savings: rust_decimal::Decimal,
    pub estimated_savings_pct: rust_decimal::Decimal,
    pub currency: String,
    pub impact: String,
    pub effort: String,
    pub risk: String,
    pub status: String,
    pub details: serde_json::Value,
    pub notes: Option<String>,
    pub implemented_by: Option<Uuid>,
    pub implemented_at: Option<DateTime<Utc>>,
    pub rule_id: Option<String>,
    pub confidence: Option<String>,
    pub terraform_code: Option<String>,
    pub resource_metadata: serde_json::Value,
    pub resource_arn: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub severity: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct RecommendationSummary {
    pub total_count: i64,
    pub pending_count: i64,
    pub implemented_count: i64,
    pub dismissed_count: i64,
    pub total_savings: f64,
    pub implemented_savings: f64,
    pub by_type: serde_json::Value,
    pub by_impact: serde_json::Value,
    pub currency: String,
}

#[derive(Debug, Deserialize)]
pub struct RecommendationQueryParams {
    pub status: Option<String>,
    #[serde(rename = "type")]
    pub rec_type: Option<String>,
    pub impact: Option<String>,
    pub provider: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRecommendationStatusRequest {
    pub status: String,
    pub notes: Option<String>,
}
