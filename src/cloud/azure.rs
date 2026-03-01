use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::models::AzureCredentials;
use super::{CloudCostItem, TestResult};

/// Azure OAuth2 token response.
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

/// Azure Cost Management query result row.
#[derive(Deserialize)]
struct CostQueryResult {
    properties: CostQueryProperties,
}

#[derive(Deserialize)]
struct CostQueryProperties {
    columns: Vec<CostQueryColumn>,
    rows: Vec<Vec<serde_json::Value>>,
}

#[derive(Deserialize)]
struct CostQueryColumn {
    name: String,
}

/// Request body for Azure Cost Management query.
#[derive(Serialize)]
struct CostQueryRequest {
    #[serde(rename = "type")]
    query_type: String,
    timeframe: String,
    #[serde(rename = "timePeriod")]
    time_period: TimePeriod,
    dataset: Dataset,
}

#[derive(Serialize)]
struct TimePeriod {
    from: String,
    to: String,
}

#[derive(Serialize)]
struct Dataset {
    granularity: String,
    aggregation: std::collections::HashMap<String, AggregationExpr>,
    grouping: Vec<GroupingExpr>,
}

#[derive(Serialize)]
struct AggregationExpr {
    name: String,
    function: String,
}

#[derive(Serialize)]
struct GroupingExpr {
    #[serde(rename = "type")]
    grouping_type: String,
    name: String,
}

/// Get an Azure AD OAuth2 access token using client credentials.
async fn get_access_token(creds: &AzureCredentials) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
        creds.tenant_id
    );

    let resp = client
        .post(&url)
        .form(&[
            ("grant_type", "client_credentials"),
            ("client_id", &creds.client_id),
            ("client_secret", &creds.client_secret),
            ("scope", "https://management.azure.com/.default"),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        let text = resp.text().await?;
        anyhow::bail!("Azure token request failed: {text}");
    }

    let token: TokenResponse = resp.json().await?;
    Ok(token.access_token)
}

/// Test Azure credentials by requesting an access token.
pub async fn test_credentials(creds: &AzureCredentials) -> TestResult {
    match get_access_token(creds).await {
        Ok(_) => TestResult {
            success: true,
            message: "Azure credentials validated successfully".into(),
        },
        Err(e) => TestResult {
            success: false,
            message: format!("Azure credential validation failed: {e}"),
        },
    }
}

/// Pull cost data from Azure Cost Management API.
pub async fn sync_costs(
    creds: &AzureCredentials,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> anyhow::Result<Vec<CloudCostItem>> {
    let token = get_access_token(creds).await?;
    let client = reqwest::Client::new();

    let url = format!(
        "https://management.azure.com/subscriptions/{}/providers/Microsoft.CostManagement/query?api-version=2023-11-01",
        creds.subscription_id
    );

    let mut aggregation = std::collections::HashMap::new();
    aggregation.insert(
        "totalCost".to_string(),
        AggregationExpr {
            name: "Cost".to_string(),
            function: "Sum".to_string(),
        },
    );

    let body = CostQueryRequest {
        query_type: "ActualCost".to_string(),
        timeframe: "Custom".to_string(),
        time_period: TimePeriod {
            from: format!("{}T00:00:00Z", start_date),
            to: format!("{}T23:59:59Z", end_date),
        },
        dataset: Dataset {
            granularity: "Daily".to_string(),
            aggregation,
            grouping: vec![
                GroupingExpr {
                    grouping_type: "Dimension".to_string(),
                    name: "ServiceName".to_string(),
                },
                GroupingExpr {
                    grouping_type: "Dimension".to_string(),
                    name: "ResourceLocation".to_string(),
                },
            ],
        },
    };

    let resp = client
        .post(&url)
        .bearer_auth(&token)
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let text = resp.text().await?;
        anyhow::bail!("Azure Cost Management query failed: {text}");
    }

    let result: CostQueryResult = resp.json().await?;

    // Find column indexes
    let columns = &result.properties.columns;
    let cost_idx = columns.iter().position(|c| c.name == "Cost").unwrap_or(0);
    let date_idx = columns.iter().position(|c| c.name == "UsageDate").unwrap_or(1);
    let service_idx = columns.iter().position(|c| c.name == "ServiceName").unwrap_or(2);
    let region_idx = columns.iter().position(|c| c.name == "ResourceLocation").unwrap_or(3);

    let mut items = Vec::new();
    for row in &result.properties.rows {
        let amount = row.get(cost_idx)
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        if amount <= 0.0 {
            continue;
        }

        let date_val = row.get(date_idx)
            .and_then(|v| v.as_i64().map(|n| n.to_string()).or_else(|| v.as_str().map(|s| s.to_string())))
            .unwrap_or_default();

        // Azure returns dates as YYYYMMDD integers
        let date = if date_val.len() == 8 {
            NaiveDate::parse_from_str(&date_val, "%Y%m%d")
                .unwrap_or_else(|_| chrono::Utc::now().date_naive())
        } else {
            NaiveDate::parse_from_str(&date_val, "%Y-%m-%d")
                .unwrap_or_else(|_| chrono::Utc::now().date_naive())
        };

        let service = row.get(service_idx)
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();

        let region = row.get(region_idx)
            .and_then(|v| v.as_str())
            .unwrap_or("global")
            .to_string();

        items.push(CloudCostItem {
            date,
            amount,
            currency: "USD".to_string(),
            service,
            account_id: creds.subscription_id.clone(),
            region,
            resource_id: String::new(),
            tags: serde_json::json!({}),
            estimated: false,
        });
    }

    info!(count = items.len(), "Fetched Azure cost data");
    Ok(items)
}
