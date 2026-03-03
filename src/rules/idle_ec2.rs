use tracing::{info, warn};

use super::{get_avg_cpu, NewRecommendation, RuleEngine};

/// Approximate hourly pricing for common EC2 instance types (us-east-1, on-demand, Linux).
fn instance_hourly_price(instance_type: &str) -> f64 {
    match instance_type {
        "t2.nano" => 0.0058,
        "t2.micro" => 0.0116,
        "t2.small" => 0.023,
        "t2.medium" => 0.0464,
        "t2.large" => 0.0928,
        "t2.xlarge" => 0.1856,
        "t2.2xlarge" => 0.3712,
        "t3.nano" => 0.0052,
        "t3.micro" => 0.0104,
        "t3.small" => 0.0208,
        "t3.medium" => 0.0416,
        "t3.large" => 0.0832,
        "t3.xlarge" => 0.1664,
        "t3.2xlarge" => 0.3328,
        "m5.large" => 0.096,
        "m5.xlarge" => 0.192,
        "m5.2xlarge" => 0.384,
        "m5.4xlarge" => 0.768,
        "m6i.large" => 0.096,
        "m6i.xlarge" => 0.192,
        "m6i.2xlarge" => 0.384,
        "c5.large" => 0.085,
        "c5.xlarge" => 0.17,
        "c5.2xlarge" => 0.34,
        "c5.4xlarge" => 0.68,
        "r5.large" => 0.126,
        "r5.xlarge" => 0.252,
        "r5.2xlarge" => 0.504,
        "i3.large" => 0.156,
        "i3.xlarge" => 0.312,
        _ => 0.10, // conservative fallback
    }
}

const CPU_THRESHOLD: f64 = 5.0;
const LOOKBACK_DAYS: i64 = 7;
const HOURS_PER_MONTH: f64 = 730.0;

pub struct IdleEc2Rule;

impl RuleEngine for IdleEc2Rule {
    fn evaluate<'a>(
        &'a self,
        config: &'a aws_config::SdkConfig,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<Vec<NewRecommendation>>> + Send + 'a>> {
        Box::pin(async move {
        let ec2 = aws_sdk_ec2::Client::new(config);
        let cw = aws_sdk_cloudwatch::Client::new(config);

        let region = config
            .region()
            .map(|r| r.to_string())
            .unwrap_or_else(|| "us-east-1".into());

        let mut recommendations = Vec::new();

        // Describe all running instances
        let resp = ec2
            .describe_instances()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("instance-state-name")
                    .values("running")
                    .build(),
            )
            .send()
            .await?;

        for reservation in resp.reservations() {
            for instance in reservation.instances() {
                let instance_id = instance.instance_id().unwrap_or_default();
                let instance_type = instance
                    .instance_type()
                    .map(|t| t.as_str().to_string())
                    .unwrap_or_else(|| "unknown".into());

                if instance_id.is_empty() {
                    continue;
                }

                let avg_cpu = match get_avg_cpu(
                    &cw,
                    "AWS/EC2",
                    "InstanceId",
                    instance_id,
                    LOOKBACK_DAYS,
                )
                .await
                {
                    Ok(Some(v)) => v,
                    Ok(None) => continue, // no data, skip
                    Err(e) => {
                        warn!(instance_id, error = %e, "Failed to get CPU metrics");
                        continue;
                    }
                };

                if avg_cpu >= CPU_THRESHOLD {
                    continue;
                }

                let hourly = instance_hourly_price(&instance_type);
                let monthly_savings = hourly * HOURS_PER_MONTH;

                let name = instance
                    .tags()
                    .iter()
                    .find(|t| t.key() == Some("Name"))
                    .and_then(|t| t.value())
                    .unwrap_or("")
                    .to_string();

                let terraform_code = format!(
                    r#"# Terminate idle EC2 instance: {instance_id} ({instance_type})
# Average CPU: {avg_cpu:.2}% over {LOOKBACK_DAYS} days
# WARNING: Create an AMI backup before terminating if needed

# If managed by Terraform, remove the resource block:
# resource "aws_instance" "this" {{ }}

# Or use AWS CLI:
# aws ec2 stop-instances --instance-ids {instance_id}
# aws ec2 terminate-instances --instance-ids {instance_id}"#
                );

                recommendations.push(NewRecommendation {
                    rec_type: "idle_resource".into(),
                    provider: "aws".into(),
                    resource_id: instance_id.to_string(),
                    resource_type: "EC2 Instance".into(),
                    region: region.clone(),
                    account_id: String::new(),
                    estimated_savings: monthly_savings,
                    estimated_savings_pct: 100.0,
                    current_config: serde_json::json!({
                        "instance_type": instance_type,
                        "state": "running",
                        "avg_cpu_7d": format!("{avg_cpu:.2}%"),
                        "name": name,
                    }),
                    recommended_config: serde_json::json!({
                        "action": "terminate",
                        "reason": format!("Average CPU {avg_cpu:.2}% < {CPU_THRESHOLD}% over {LOOKBACK_DAYS} days"),
                    }),
                    impact: "high".into(),
                    effort: "low".into(),
                    risk: "medium".into(),
                    rule_id: "idle-ec2".into(),
                    severity: if monthly_savings > 100.0 {
                        "high".into()
                    } else {
                        "medium".into()
                    },
                    details: serde_json::json!({}),
                    terraform_code: Some(terraform_code),
                });
            }
        }

        info!(count = recommendations.len(), "Idle EC2 rule completed");
        Ok(recommendations)
        })
    }
}
