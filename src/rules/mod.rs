pub mod idle_ec2;
pub mod oversized_rds;
pub mod unattached_ebs;
pub mod old_snapshots;

use std::future::Future;
use std::pin::Pin;

use aws_credential_types::provider::SharedCredentialsProvider;
use aws_credential_types::Credentials;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::models::AwsCredentials;

/// A recommendation produced by a rule before it is persisted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewRecommendation {
    pub rec_type: String,
    pub provider: String,
    pub resource_id: String,
    pub resource_type: String,
    pub region: String,
    pub account_id: String,
    pub estimated_savings: f64,
    pub estimated_savings_pct: f64,
    pub current_config: serde_json::Value,
    pub recommended_config: serde_json::Value,
    pub impact: String,
    pub effort: String,
    pub risk: String,
    pub rule_id: String,
    pub severity: String,
    pub details: serde_json::Value,
}

/// Trait implemented by each recommendation rule.
pub trait RuleEngine: Send + Sync {
    /// Evaluate the rule against a cloud provider and return new recommendations.
    fn evaluate<'a>(
        &'a self,
        config: &'a aws_config::SdkConfig,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<NewRecommendation>>> + Send + 'a>>;
}

/// Build a shared AWS SDK config from stored credentials.
pub fn build_aws_config(creds: &AwsCredentials) -> aws_config::SdkConfig {
    let credentials = Credentials::new(
        &creds.access_key_id,
        &creds.secret_key,
        None,
        None,
        "finopsmind",
    );
    let region = aws_config::Region::new(creds.region.clone());
    aws_config::SdkConfig::builder()
        .region(region)
        .credentials_provider(SharedCredentialsProvider::new(credentials))
        .behavior_version(aws_config::BehaviorVersion::latest())
        .build()
}

/// Get the average value of a CloudWatch CPUUtilization metric over the last N days.
pub async fn get_avg_cpu(
    cw_client: &aws_sdk_cloudwatch::Client,
    namespace: &str,
    dimension_name: &str,
    dimension_value: &str,
    days: i64,
) -> anyhow::Result<Option<f64>> {
    let end = Utc::now();
    let start = end - chrono::Duration::days(days);

    let resp = cw_client
        .get_metric_statistics()
        .namespace(namespace)
        .metric_name("CPUUtilization")
        .dimensions(
            aws_sdk_cloudwatch::types::Dimension::builder()
                .name(dimension_name)
                .value(dimension_value)
                .build(),
        )
        .start_time(aws_sdk_cloudwatch::primitives::DateTime::from_millis(
            start.timestamp_millis(),
        ))
        .end_time(aws_sdk_cloudwatch::primitives::DateTime::from_millis(
            end.timestamp_millis(),
        ))
        .period(86400) // 1-day granularity
        .statistics(aws_sdk_cloudwatch::types::Statistic::Average)
        .send()
        .await?;

    let datapoints = resp.datapoints();
    if datapoints.is_empty() {
        return Ok(None);
    }

    let sum: f64 = datapoints.iter().filter_map(|dp| dp.average()).sum();
    let count = datapoints.iter().filter(|dp| dp.average().is_some()).count();

    if count == 0 {
        return Ok(None);
    }

    Ok(Some(sum / count as f64))
}

/// Check whether a DateTime is older than `days` from now.
#[allow(dead_code)]
pub fn is_older_than_days(dt: &DateTime<Utc>, days: i64) -> bool {
    let cutoff = Utc::now() - chrono::Duration::days(days);
    *dt < cutoff
}
