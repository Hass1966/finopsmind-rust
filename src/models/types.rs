use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDate};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "varchar")]
#[sqlx(rename_all = "lowercase")]
pub enum CloudProviderType {
    #[serde(rename = "aws")]
    #[sqlx(rename = "aws")]
    Aws,
    #[serde(rename = "azure")]
    #[sqlx(rename = "azure")]
    Azure,
    #[serde(rename = "gcp")]
    #[sqlx(rename = "gcp")]
    Gcp,
}

impl std::fmt::Display for CloudProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Aws => write!(f, "aws"),
            Self::Azure => write!(f, "azure"),
            Self::Gcp => write!(f, "gcp"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub page: i64,
    pub page_size: i64,
    pub total: i64,
}

impl Pagination {
    pub fn new(page: i64, page_size: i64, total: i64) -> Self {
        Self { page, page_size, total }
    }

    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.page_size
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub data: Vec<T>,
    pub pagination: Pagination,
}

pub type Tags = HashMap<String, String>;
