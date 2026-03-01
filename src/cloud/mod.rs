pub mod aws;
pub mod azure;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// A normalized cost record from any cloud provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudCostItem {
    pub date: NaiveDate,
    pub amount: f64,
    pub currency: String,
    pub service: String,
    pub account_id: String,
    pub region: String,
    pub resource_id: String,
    pub tags: serde_json::Value,
    pub estimated: bool,
}

/// Result of testing provider credentials.
#[derive(Debug)]
pub struct TestResult {
    pub success: bool,
    pub message: String,
}
