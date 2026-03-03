use tracing::{info, warn};

use super::{get_avg_cpu, NewRecommendation, RuleEngine};

/// Map an RDS instance class to its approximate monthly on-demand cost (us-east-1, Single-AZ).
fn rds_monthly_price(class: &str) -> f64 {
    match class {
        "db.t3.micro" => 12.41,
        "db.t3.small" => 24.82,
        "db.t3.medium" => 49.64,
        "db.t3.large" => 99.28,
        "db.t3.xlarge" => 198.56,
        "db.t3.2xlarge" => 397.12,
        "db.m5.large" => 124.10,
        "db.m5.xlarge" => 248.20,
        "db.m5.2xlarge" => 496.40,
        "db.m5.4xlarge" => 992.80,
        "db.m6i.large" => 124.10,
        "db.m6i.xlarge" => 248.20,
        "db.m6i.2xlarge" => 496.40,
        "db.r5.large" => 175.20,
        "db.r5.xlarge" => 350.40,
        "db.r5.2xlarge" => 700.80,
        "db.r5.4xlarge" => 1401.60,
        "db.r6i.large" => 175.20,
        "db.r6i.xlarge" => 350.40,
        _ => 100.0, // conservative fallback
    }
}

/// Return the next smaller RDS instance class within the same family, if any.
fn downsize_class(class: &str) -> Option<&'static str> {
    match class {
        "db.t3.2xlarge" => Some("db.t3.xlarge"),
        "db.t3.xlarge" => Some("db.t3.large"),
        "db.t3.large" => Some("db.t3.medium"),
        "db.t3.medium" => Some("db.t3.small"),
        "db.t3.small" => Some("db.t3.micro"),
        "db.m5.4xlarge" => Some("db.m5.2xlarge"),
        "db.m5.2xlarge" => Some("db.m5.xlarge"),
        "db.m5.xlarge" => Some("db.m5.large"),
        "db.m6i.2xlarge" => Some("db.m6i.xlarge"),
        "db.m6i.xlarge" => Some("db.m6i.large"),
        "db.r5.4xlarge" => Some("db.r5.2xlarge"),
        "db.r5.2xlarge" => Some("db.r5.xlarge"),
        "db.r5.xlarge" => Some("db.r5.large"),
        "db.r6i.xlarge" => Some("db.r6i.large"),
        _ => None,
    }
}

const CPU_THRESHOLD: f64 = 10.0;
const LOOKBACK_DAYS: i64 = 7;

pub struct OversizedRdsRule;

impl RuleEngine for OversizedRdsRule {
    fn evaluate<'a>(
        &'a self,
        config: &'a aws_config::SdkConfig,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<Vec<NewRecommendation>>> + Send + 'a>> {
        Box::pin(async move {
        let rds = aws_sdk_rds::Client::new(config);
        let cw = aws_sdk_cloudwatch::Client::new(config);

        let region = config
            .region()
            .map(|r| r.to_string())
            .unwrap_or_else(|| "us-east-1".into());

        let mut recommendations = Vec::new();

        let resp = rds.describe_db_instances().send().await?;

        for db in resp.db_instances() {
            let db_id = db.db_instance_identifier().unwrap_or_default();
            let db_class = db.db_instance_class().unwrap_or_default();
            let engine = db.engine().unwrap_or_default();
            let status = db.db_instance_status().unwrap_or_default();

            if db_id.is_empty() || status != "available" {
                continue;
            }

            let smaller = match downsize_class(db_class) {
                Some(s) => s,
                None => continue, // already smallest or unknown class
            };

            let avg_cpu =
                match get_avg_cpu(&cw, "AWS/RDS", "DBInstanceIdentifier", db_id, LOOKBACK_DAYS)
                    .await
                {
                    Ok(Some(v)) => v,
                    Ok(None) => continue,
                    Err(e) => {
                        warn!(db_id, error = %e, "Failed to get RDS CPU metrics");
                        continue;
                    }
                };

            if avg_cpu >= CPU_THRESHOLD {
                continue;
            }

            let current_cost = rds_monthly_price(db_class);
            let smaller_cost = rds_monthly_price(smaller);
            let savings = current_cost - smaller_cost;

            if savings <= 0.0 {
                continue;
            }

            let savings_pct = (savings / current_cost) * 100.0;
            let multi_az = db.multi_az();
            let storage = db.allocated_storage().unwrap_or(0);

            recommendations.push(NewRecommendation {
                rec_type: "rightsizing".into(),
                provider: "aws".into(),
                resource_id: db_id.to_string(),
                resource_type: "RDS Instance".into(),
                region: region.clone(),
                account_id: String::new(),
                estimated_savings: savings,
                estimated_savings_pct: savings_pct,
                current_config: serde_json::json!({
                    "instance_class": db_class,
                    "engine": engine,
                    "avg_cpu_7d": format!("{avg_cpu:.2}%"),
                    "multi_az": multi_az,
                    "allocated_storage_gb": storage,
                }),
                recommended_config: serde_json::json!({
                    "instance_class": smaller,
                    "action": "downsize",
                    "reason": format!("Average CPU {avg_cpu:.2}% < {CPU_THRESHOLD}% over {LOOKBACK_DAYS} days"),
                }),
                impact: "high".into(),
                effort: "medium".into(),
                risk: "medium".into(),
                rule_id: "oversized-rds".into(),
                severity: if savings > 200.0 {
                    "high".into()
                } else {
                    "medium".into()
                },
                details: serde_json::json!({}),
            });
        }

        info!(count = recommendations.len(), "Oversized RDS rule completed");
        Ok(recommendations)
        })
    }
}
