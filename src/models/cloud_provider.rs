use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CloudProviderConfig {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub provider_type: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub credentials: Option<Vec<u8>>,
    pub enabled: bool,
    pub status: String,
    pub status_message: Option<String>,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProviderRequest {
    pub provider_type: String,
    pub name: String,
    pub credentials: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderRequest {
    pub name: Option<String>,
    pub credentials: Option<serde_json::Value>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AwsCredentials {
    pub access_key_id: String,
    pub secret_key: String,
    pub region: String,
    pub assume_role_arn: Option<String>,
    pub external_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AzureCredentials {
    pub tenant_id: String,
    pub client_id: String,
    pub client_secret: String,
    pub subscription_id: String,
}
