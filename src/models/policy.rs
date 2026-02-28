use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Policy {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    #[sqlx(rename = "type")]
    pub policy_type: String,
    pub enforcement_mode: String,
    pub enabled: bool,
    pub conditions: serde_json::Value,
    pub providers: serde_json::Value,
    pub environments: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub last_evaluated_at: Option<DateTime<Utc>>,
    pub violation_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PolicyViolation {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub policy_id: Uuid,
    pub policy_name: String,
    pub status: String,
    pub provider: String,
    pub account_id: String,
    pub region: String,
    pub resource_id: String,
    pub resource_type: String,
    pub description: Option<String>,
    pub severity: String,
    pub details: serde_json::Value,
    pub detected_at: DateTime<Utc>,
    pub remediated_at: Option<DateTime<Utc>>,
    pub exempted_by: Option<Uuid>,
    pub exempt_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePolicyRequest {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub policy_type: String,
    #[serde(default = "default_enforcement")]
    pub enforcement_mode: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub conditions: serde_json::Value,
    #[serde(default)]
    pub providers: serde_json::Value,
    #[serde(default)]
    pub environments: serde_json::Value,
}

fn default_enforcement() -> String {
    "alert_only".into()
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub struct PolicySummary {
    pub total_policies: i64,
    pub enabled_policies: i64,
    pub total_violations: i64,
    pub open_violations: i64,
    pub by_type: serde_json::Value,
    pub by_severity: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct PolicyQueryParams {
    pub policy_type: Option<String>,
    pub enabled: Option<bool>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ViolationQueryParams {
    pub policy_id: Option<Uuid>,
    pub status: Option<String>,
    pub severity: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
