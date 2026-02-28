use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub settings: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationSettings {
    #[serde(default = "default_currency")]
    pub default_currency: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    #[serde(default = "default_fiscal_year")]
    pub fiscal_year_start: i32,
    #[serde(default = "default_true")]
    pub alerts_enabled: bool,
    #[serde(default)]
    pub slack_webhook_url: Option<String>,
    #[serde(default)]
    pub email_recipients: Vec<String>,
}

fn default_currency() -> String {
    "USD".into()
}
fn default_timezone() -> String {
    "UTC".into()
}
fn default_fiscal_year() -> i32 {
    1
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Alert {
    pub id: Uuid,
    pub organization_id: Uuid,
    #[sqlx(rename = "type")]
    pub alert_type: String,
    pub severity: String,
    pub status: String,
    pub title: String,
    pub message: Option<String>,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub metadata: serde_json::Value,
    pub triggered_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
