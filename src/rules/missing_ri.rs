use tracing::info;

use super::{NewRecommendation, RuleEngine};

/// Approximate monthly on-demand price for common EC2 instance types (us-east-1, Linux).
fn monthly_on_demand(instance_type: &str) -> f64 {
    let hourly = match instance_type {
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
        _ => 0.10,
    };
    hourly * 730.0
}

/// Estimated RI savings percentage (1-year, no upfront, standard).
const RI_SAVINGS_PCT: f64 = 0.35;
const MIN_RUNNING_DAYS: i64 = 30;

pub struct MissingRiRule;

impl RuleEngine for MissingRiRule {
    fn evaluate<'a>(
        &'a self,
        config: &'a aws_config::SdkConfig,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<Vec<NewRecommendation>>> + Send + 'a>> {
        Box::pin(async move {
        let ec2 = aws_sdk_ec2::Client::new(config);

        let region = config
            .region()
            .map(|r| r.to_string())
            .unwrap_or_else(|| "us-east-1".into());

        let mut recommendations = Vec::new();

        // Get all running instances
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

        // Get existing reserved instances to exclude already-covered types
        let ri_resp = ec2.describe_reserved_instances()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("state")
                    .values("active")
                    .build(),
            )
            .send()
            .await;

        let mut reserved_types: std::collections::HashSet<String> = std::collections::HashSet::new();
        if let Ok(ri_resp) = &ri_resp {
            for ri in ri_resp.reserved_instances() {
                if let Some(it) = ri.instance_type() {
                    reserved_types.insert(it.as_str().to_string());
                }
            }
        }

        let now = chrono::Utc::now();

        for reservation in resp.reservations() {
            for instance in reservation.instances() {
                let instance_id = instance.instance_id().unwrap_or_default();
                let instance_type = instance
                    .instance_type()
                    .map(|t| t.as_str().to_string())
                    .unwrap_or_else(|| "unknown".into());

                if instance_id.is_empty() || instance_type == "unknown" {
                    continue;
                }

                // Skip if this instance type is already covered by an RI
                if reserved_types.contains(&instance_type) {
                    continue;
                }

                // Check launch time to see if running 30+ days
                let launch_time = match instance.launch_time() {
                    Some(t) => t,
                    None => continue,
                };

                let launched_at = match chrono::DateTime::from_timestamp(
                    launch_time.secs(),
                    launch_time.subsec_nanos(),
                ) {
                    Some(dt) => dt,
                    None => continue,
                };

                let running_days = (now - launched_at).num_days();
                if running_days < MIN_RUNNING_DAYS {
                    continue;
                }

                let monthly_cost = monthly_on_demand(&instance_type);
                let monthly_savings = monthly_cost * RI_SAVINGS_PCT;

                let name = instance
                    .tags()
                    .iter()
                    .find(|t| t.key() == Some("Name"))
                    .and_then(|t| t.value())
                    .unwrap_or("")
                    .to_string();

                let terraform_code = format!(
                    r#"# Purchase Reserved Instance for {instance_type} (1-year, no upfront)
resource "aws_ec2_capacity_reservation" "{instance_id}_ri" {{
  instance_type     = "{instance_type}"
  instance_platform = "Linux/UNIX"
  availability_zone = "{region}a"
  instance_count    = 1
}}

# Alternatively, use AWS CLI to purchase RI:
# aws ec2 purchase-reserved-instances-offering \
#   --instance-type {instance_type} \
#   --instance-count 1 \
#   --reserved-instances-offering-id <offering-id>"#
                );

                recommendations.push(NewRecommendation {
                    rec_type: "reserved_instance".into(),
                    provider: "aws".into(),
                    resource_id: instance_id.to_string(),
                    resource_type: "EC2 Instance".into(),
                    region: region.clone(),
                    account_id: String::new(),
                    estimated_savings: monthly_savings,
                    estimated_savings_pct: RI_SAVINGS_PCT * 100.0,
                    current_config: serde_json::json!({
                        "instance_type": instance_type,
                        "pricing": "on-demand",
                        "running_days": running_days,
                        "monthly_cost": format!("${monthly_cost:.2}"),
                        "name": name,
                    }),
                    recommended_config: serde_json::json!({
                        "action": "purchase_ri",
                        "term": "1-year",
                        "payment": "no-upfront",
                        "reason": format!("Instance running on-demand for {running_days} days, RI saves ~{:.0}%", RI_SAVINGS_PCT * 100.0),
                    }),
                    impact: "high".into(),
                    effort: "low".into(),
                    risk: "low".into(),
                    rule_id: "missing-ri".into(),
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

        info!(count = recommendations.len(), "Missing RI rule completed");
        Ok(recommendations)
        })
    }
}
