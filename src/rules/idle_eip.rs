use tracing::info;

use super::{NewRecommendation, RuleEngine};

/// AWS charges $3.65/month ($0.005/hr) for Elastic IPs not associated with a running instance.
const EIP_MONTHLY_COST: f64 = 3.65;

pub struct IdleEipRule;

impl RuleEngine for IdleEipRule {
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

        let resp = ec2.describe_addresses().send().await?;

        for addr in resp.addresses() {
            let allocation_id = addr.allocation_id().unwrap_or_default();
            let public_ip = addr.public_ip().unwrap_or_default();

            if allocation_id.is_empty() {
                continue;
            }

            // An EIP is idle if it has no association (not attached to an instance or NAT gateway)
            if addr.association_id().is_some() {
                continue;
            }

            let name = addr.tags()
                .iter()
                .find(|t| t.key() == Some("Name"))
                .and_then(|t| t.value())
                .unwrap_or("")
                .to_string();

            let terraform_code = format!(
                r#"# Release idle Elastic IP: {public_ip} ({allocation_id})
# WARNING: Releasing an EIP makes the address available to other accounts.
# Ensure no DNS records point to this IP before releasing.

# If managed by Terraform, remove the resource block:
# resource "aws_eip" "this" {{ }}

# Or use AWS CLI:
# aws ec2 release-address --allocation-id {allocation_id}"#
            );

            recommendations.push(NewRecommendation {
                rec_type: "unused_resource".into(),
                provider: "aws".into(),
                resource_id: allocation_id.to_string(),
                resource_type: "Elastic IP".into(),
                region: region.clone(),
                account_id: String::new(),
                estimated_savings: EIP_MONTHLY_COST,
                estimated_savings_pct: 100.0,
                current_config: serde_json::json!({
                    "public_ip": public_ip,
                    "allocation_id": allocation_id,
                    "association": "none",
                    "name": name,
                }),
                recommended_config: serde_json::json!({
                    "action": "release",
                    "reason": "Elastic IP is not associated with any running instance or NAT gateway",
                }),
                impact: "low".into(),
                effort: "low".into(),
                risk: "low".into(),
                rule_id: "idle-eip".into(),
                severity: "low".into(),
                details: serde_json::json!({}),
                terraform_code: Some(terraform_code),
            });
        }

        info!(count = recommendations.len(), "Idle EIP rule completed");
        Ok(recommendations)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eip_monthly_cost() {
        // $0.005/hr * 730 hrs ≈ $3.65
        let hourly = 0.005;
        let monthly = hourly * 730.0;
        assert!((monthly - EIP_MONTHLY_COST).abs() < 0.01);
    }
}
