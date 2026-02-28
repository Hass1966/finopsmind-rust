use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub first_name: String,
    pub last_name: String,
    pub role: String,
    #[serde(skip_serializing)]
    pub api_key_hash: Option<String>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name).trim().to_string()
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct SignupRequest {
    pub email: String,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
    pub organization_name: String,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub token: String,
    pub user: UserInfo,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: Uuid,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub role: String,
    pub organization_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyResponse {
    pub key: String,
    pub message: String,
}
