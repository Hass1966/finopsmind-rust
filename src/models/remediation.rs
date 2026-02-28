use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RemediationAction {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub recommendation_id: Option<Uuid>,
    #[sqlx(rename = "type")]
    pub action_type: String,
    pub status: String,
    pub provider: String,
    pub account_id: String,
    pub region: String,
    pub resource_id: String,
    pub resource_type: String,
    pub description: Option<String>,
    pub current_state: serde_json::Value,
    pub desired_state: serde_json::Value,
    pub estimated_savings: rust_decimal::Decimal,
    pub currency: String,
    pub risk: String,
    pub auto_approved: bool,
    pub approval_rule: Option<String>,
    pub requested_by: Option<Uuid>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub executed_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub rolled_back_at: Option<DateTime<Utc>>,
    pub failure_reason: Option<String>,
    pub rollback_data: serde_json::Value,
    pub audit_log: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ProposeRemediationRequest {
    pub recommendation_id: Option<Uuid>,
    #[serde(rename = "type")]
    pub action_type: String,
    pub provider: String,
    pub account_id: String,
    pub region: String,
    pub resource_id: String,
    pub resource_type: String,
    pub description: Option<String>,
    pub current_state: Option<serde_json::Value>,
    pub desired_state: Option<serde_json::Value>,
    pub estimated_savings: Option<f64>,
    pub risk: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RejectRemediationRequest {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AutoApprovalRule {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub conditions: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAutoApprovalRuleRequest {
    pub name: String,
    pub enabled: Option<bool>,
    pub conditions: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAutoApprovalRuleRequest {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub conditions: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct RemediationSummary {
    pub total: i64,
    pub pending_approval: i64,
    pub approved: i64,
    pub executing: i64,
    pub completed: i64,
    pub failed: i64,
    pub rolled_back: i64,
    pub total_savings: f64,
    pub currency: String,
}
