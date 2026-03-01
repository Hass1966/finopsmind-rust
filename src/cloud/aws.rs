use aws_credential_types::provider::SharedCredentialsProvider;
use aws_credential_types::Credentials;
use aws_sdk_costexplorer as ce;
use chrono::NaiveDate;
use tracing::info;

use crate::models::AwsCredentials;
use super::{CloudCostItem, TestResult};

/// Build an AWS Cost Explorer client from stored credentials.
fn build_client(creds: &AwsCredentials) -> ce::Client {
    let credentials = Credentials::new(
        &creds.access_key_id,
        &creds.secret_key,
        None,
        None,
        "finopsmind",
    );
    let region = aws_config::Region::new(creds.region.clone());
    let config = aws_config::SdkConfig::builder()
        .region(region)
        .credentials_provider(SharedCredentialsProvider::new(credentials))
        .behavior_version(aws_config::BehaviorVersion::latest())
        .build();
    ce::Client::new(&config)
}

/// Test AWS credentials by calling GetCostAndUsage for a 1-day window.
pub async fn test_credentials(creds: &AwsCredentials) -> TestResult {
    let client = build_client(creds);
    let today = chrono::Utc::now().date_naive();
    let yesterday = today - chrono::Duration::days(1);

    match client
        .get_cost_and_usage()
        .time_period(
            ce::types::DateInterval::builder()
                .start(yesterday.to_string())
                .end(today.to_string())
                .build()
                .unwrap(),
        )
        .granularity(ce::types::Granularity::Daily)
        .metrics("UnblendedCost")
        .send()
        .await
    {
        Ok(_) => TestResult {
            success: true,
            message: "AWS credentials validated successfully".into(),
        },
        Err(e) => TestResult {
            success: false,
            message: format!("AWS credential validation failed: {e}"),
        },
    }
}

/// Pull cost data from AWS Cost Explorer for the given date range.
pub async fn sync_costs(
    creds: &AwsCredentials,
    start_date: NaiveDate,
    end_date: NaiveDate,
    account_id: &str,
) -> anyhow::Result<Vec<CloudCostItem>> {
    let client = build_client(creds);
    let mut items = Vec::new();

    let resp = client
        .get_cost_and_usage()
        .time_period(
            ce::types::DateInterval::builder()
                .start(start_date.to_string())
                .end(end_date.to_string())
                .build()
                .unwrap(),
        )
        .granularity(ce::types::Granularity::Daily)
        .metrics("UnblendedCost")
        .group_by(
            ce::types::GroupDefinition::builder()
                .r#type(ce::types::GroupDefinitionType::Dimension)
                .key("SERVICE")
                .build(),
        )
        .group_by(
            ce::types::GroupDefinition::builder()
                .r#type(ce::types::GroupDefinitionType::Dimension)
                .key("REGION")
                .build(),
        )
        .send()
        .await?;

    for result in resp.results_by_time() {
        let period = result.time_period();
        let date_str = period.map(|p| p.start()).unwrap_or("");
        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .unwrap_or_else(|_| chrono::Utc::now().date_naive());
        let estimated = result.estimated();

        for group in result.groups() {
            let group_keys = group.keys();
            let service = group_keys.first().map(String::as_str).unwrap_or("Unknown");
            let region = group_keys.get(1).map(String::as_str).unwrap_or("global");

            if let Some(metrics_map) = group.metrics() {
                if let Some(cost_metric) = metrics_map.get("UnblendedCost") {
                    let amount: f64 = cost_metric
                        .amount()
                        .unwrap_or("0")
                        .parse()
                        .unwrap_or(0.0);
                    let currency = cost_metric
                        .unit()
                        .unwrap_or("USD")
                        .to_string();

                    if amount > 0.0 {
                        items.push(CloudCostItem {
                            date,
                            amount,
                            currency,
                            service: service.to_string(),
                            account_id: account_id.to_string(),
                            region: region.to_string(),
                            resource_id: String::new(),
                            tags: serde_json::json!({}),
                            estimated,
                        });
                    }
                }
            }
        }
    }

    info!(count = items.len(), "Fetched AWS cost data");
    Ok(items)
}
