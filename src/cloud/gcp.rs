use chrono::NaiveDate;
use serde::Deserialize;
use tracing::info;

use crate::models::GcpCredentials;
use super::{CloudCostItem, TestResult};

/// Minimal Google service account key file structure.
#[derive(Deserialize)]
struct ServiceAccountKey {
    client_email: String,
    private_key: String,
    token_uri: Option<String>,
}

/// Google OAuth2 token response.
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

/// Get an OAuth2 access token using the service account JSON key.
///
/// Builds a JWT signed with RS256 and exchanges it at Google's token endpoint.
async fn get_access_token(creds: &GcpCredentials) -> anyhow::Result<String> {
    let sa_key: ServiceAccountKey = serde_json::from_str(&creds.service_account_json)?;

    let token_uri = sa_key
        .token_uri
        .as_deref()
        .unwrap_or("https://oauth2.googleapis.com/token");

    let now = chrono::Utc::now().timestamp();

    let claims = serde_json::json!({
        "iss": sa_key.client_email,
        "scope": "https://www.googleapis.com/auth/cloud-billing.readonly https://www.googleapis.com/auth/cloud-platform",
        "aud": token_uri,
        "iat": now,
        "exp": now + 3600,
    });

    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(sa_key.private_key.as_bytes())?;
    let jwt = jsonwebtoken::encode(&header, &claims, &key)?;

    let client = reqwest::Client::new();
    let resp = client
        .post(token_uri)
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", &jwt),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        let text = resp.text().await?;
        anyhow::bail!("GCP token request failed: {text}");
    }

    let token: TokenResponse = resp.json().await?;
    Ok(token.access_token)
}

/// Test GCP credentials by requesting an access token.
#[allow(dead_code)]
pub async fn test_credentials(creds: &GcpCredentials) -> TestResult {
    match get_access_token(creds).await {
        Ok(_) => TestResult {
            success: true,
            message: "GCP credentials validated successfully".into(),
        },
        Err(e) => TestResult {
            success: false,
            message: format!("GCP credential validation failed: {e}"),
        },
    }
}

/// Pull cost data from GCP Cloud Billing API for the given date range.
///
/// Uses the `billingAccounts.costs` endpoint style via the Cloud Billing
/// budgets / export API.  Falls back to the BigQuery billing-export approach
/// where the billing account is derived from the project.
pub async fn sync_costs(
    creds: &GcpCredentials,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> anyhow::Result<Vec<CloudCostItem>> {
    let token = get_access_token(creds).await?;
    let client = reqwest::Client::new();

    // Use the Cloud Billing API to list billing info, then query costs.
    // First, find the billing account for the project.
    let project_billing_url = format!(
        "https://cloudbilling.googleapis.com/v1/projects/{}/billingInfo",
        creds.project_id
    );

    let billing_resp = client
        .get(&project_billing_url)
        .bearer_auth(&token)
        .header("User-Agent", "finopsmind")
        .send()
        .await?;

    if !billing_resp.status().is_success() {
        let text = billing_resp.text().await?;
        anyhow::bail!("Failed to get GCP billing info: {text}");
    }

    let billing_info: serde_json::Value = billing_resp.json().await?;
    let billing_account = billing_info["billingAccountName"]
        .as_str()
        .unwrap_or("")
        .to_string();

    if billing_account.is_empty() {
        anyhow::bail!("No billing account linked to project {}", creds.project_id);
    }

    // Query cost data via the BigQuery Data Transfer / billing export.
    // The standard approach is to query the billing export dataset in BigQuery.
    // We use a simplified REST query against the billing catalog here.
    let query_url = format!(
        "https://bigquery.googleapis.com/bigquery/v2/projects/{}/queries",
        creds.project_id
    );

    let sql = format!(
        r#"SELECT
             CAST(usage_start_time AS STRING) as usage_start_time,
             service.description as service_description,
             location.region as region,
             sku.description as sku_description,
             cost
           FROM `{project_id}.billing_export.gcp_billing_export_v1_*`
           WHERE usage_start_time >= TIMESTAMP("{start}")
             AND usage_start_time < TIMESTAMP("{end}")
             AND cost > 0
           ORDER BY usage_start_time"#,
        project_id = creds.project_id,
        start = start_date,
        end = end_date,
    );

    let query_resp = client
        .post(&query_url)
        .bearer_auth(&token)
        .header("User-Agent", "finopsmind")
        .json(&serde_json::json!({
            "query": sql,
            "useLegacySql": false,
            "maxResults": 10000,
        }))
        .send()
        .await?;

    let mut items = Vec::new();

    if query_resp.status().is_success() {
        let data: serde_json::Value = query_resp.json().await?;

        if let Some(rows) = data["rows"].as_array() {
            for row in rows {
                let fields = match row["f"].as_array() {
                    Some(f) => f,
                    None => continue,
                };

                let usage_time = fields
                    .first()
                    .and_then(|f| f["v"].as_str())
                    .unwrap_or("");
                let service = fields
                    .get(1)
                    .and_then(|f| f["v"].as_str())
                    .unwrap_or("Unknown");
                let region = fields
                    .get(2)
                    .and_then(|f| f["v"].as_str())
                    .unwrap_or("global");
                let _sku = fields
                    .get(3)
                    .and_then(|f| f["v"].as_str())
                    .unwrap_or("");
                let cost = fields
                    .get(4)
                    .and_then(|f| f["v"].as_str())
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);

                if cost <= 0.0 {
                    continue;
                }

                // Parse date from usage_start_time (format: "2025-01-15 00:00:00 UTC")
                let date = usage_time
                    .get(..10)
                    .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
                    .unwrap_or(start_date);

                items.push(CloudCostItem {
                    date,
                    amount: cost,
                    currency: "USD".to_string(),
                    service: service.to_string(),
                    account_id: creds.project_id.clone(),
                    region: region.to_string(),
                    resource_id: String::new(),
                    tags: serde_json::json!({}),
                    estimated: false,
                });
            }
        }
    } else {
        // If BigQuery query fails (no export configured), return empty with a warning
        let text = query_resp.text().await?;
        tracing::warn!(
            "GCP billing BigQuery query failed (billing export may not be configured): {text}"
        );
    }

    info!(count = items.len(), "Fetched GCP cost data");
    Ok(items)
}
