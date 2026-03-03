use tracing::info;

use super::{NewRecommendation, RuleEngine};

/// Approximate monthly cost per GB for EBS volume types.
fn ebs_price_per_gb(volume_type: &str) -> f64 {
    match volume_type {
        "gp2" => 0.10,
        "gp3" => 0.08,
        "io1" => 0.125,
        "io2" => 0.125,
        "st1" => 0.045,
        "sc1" => 0.015,
        "standard" => 0.05,
        _ => 0.10,
    }
}

pub struct UnattachedEbsRule;

impl RuleEngine for UnattachedEbsRule {
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

        // Filter for volumes in 'available' state (not attached to any instance)
        let resp = ec2
            .describe_volumes()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("status")
                    .values("available")
                    .build(),
            )
            .send()
            .await?;

        for volume in resp.volumes() {
            let vol_id = volume.volume_id().unwrap_or_default();
            let size_gb = volume.size().unwrap_or(0);
            let vol_type = volume
                .volume_type()
                .map(|t| t.as_str().to_string())
                .unwrap_or_else(|| "gp2".into());

            if vol_id.is_empty() || size_gb == 0 {
                continue;
            }

            let price = ebs_price_per_gb(&vol_type);
            let monthly_savings = size_gb as f64 * price;

            let name = volume
                .tags()
                .iter()
                .find(|t| t.key() == Some("Name"))
                .and_then(|t| t.value())
                .unwrap_or("")
                .to_string();

            let created = volume
                .create_time()
                .and_then(|t| {
                    chrono::DateTime::from_timestamp(t.secs(), t.subsec_nanos())
                })
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_default();

            let terraform_code = format!(
                r#"# Delete unattached EBS volume: {vol_id} ({vol_type}, {size_gb} GB)
# Create a snapshot first if the data may be needed:
# aws ec2 create-snapshot --volume-id {vol_id} --description "Backup before deletion"

# If managed by Terraform, remove the resource block:
# resource "aws_ebs_volume" "this" {{ }}

# Or use AWS CLI:
# aws ec2 delete-volume --volume-id {vol_id}"#
            );

            recommendations.push(NewRecommendation {
                rec_type: "unused_resource".into(),
                provider: "aws".into(),
                resource_id: vol_id.to_string(),
                resource_type: "EBS Volume".into(),
                region: region.clone(),
                account_id: String::new(),
                estimated_savings: monthly_savings,
                estimated_savings_pct: 100.0,
                current_config: serde_json::json!({
                    "volume_type": vol_type,
                    "size_gb": size_gb,
                    "state": "available",
                    "name": name,
                    "created": created,
                }),
                recommended_config: serde_json::json!({
                    "action": "delete",
                    "reason": "Volume is not attached to any instance",
                    "pre_step": "Create a snapshot before deleting if data may be needed",
                }),
                impact: "medium".into(),
                effort: "low".into(),
                risk: "low".into(),
                rule_id: "unattached-ebs".into(),
                severity: if monthly_savings > 50.0 {
                    "medium".into()
                } else {
                    "low".into()
                },
                details: serde_json::json!({}),
                terraform_code: Some(terraform_code),
            });
        }

        info!(count = recommendations.len(), "Unattached EBS rule completed");
        Ok(recommendations)
        })
    }
}

/// Filter volumes to only those in 'available' (unattached) state.
#[cfg(test)]
fn filter_unattached(volumes: &[(String, i32, String, String)]) -> Vec<&(String, i32, String, String)> {
    volumes
        .iter()
        .filter(|(_, _, _, state)| state == "available")
        .collect()
}

/// Calculate monthly savings for an unattached EBS volume.
#[cfg(test)]
fn calculate_ebs_savings(size_gb: i32, volume_type: &str) -> f64 {
    size_gb as f64 * ebs_price_per_gb(volume_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_unattached_volumes() {
        let volumes = vec![
            ("vol-001".into(), 100, "gp2".into(), "available".into()),
            ("vol-002".into(), 50, "gp3".into(), "in-use".into()),
            ("vol-003".into(), 200, "io1".into(), "available".into()),
            ("vol-004".into(), 30, "st1".into(), "in-use".into()),
        ];

        let unattached = filter_unattached(&volumes);
        assert_eq!(unattached.len(), 2);
        assert_eq!(unattached[0].0, "vol-001");
        assert_eq!(unattached[1].0, "vol-003");
    }

    #[test]
    fn test_filter_all_attached() {
        let volumes = vec![
            ("vol-001".into(), 100, "gp2".into(), "in-use".into()),
            ("vol-002".into(), 50, "gp3".into(), "in-use".into()),
        ];

        let unattached = filter_unattached(&volumes);
        assert!(unattached.is_empty());
    }

    #[test]
    fn test_filter_all_unattached() {
        let volumes = vec![
            ("vol-001".into(), 100, "gp2".into(), "available".into()),
        ];

        let unattached = filter_unattached(&volumes);
        assert_eq!(unattached.len(), 1);
    }

    #[test]
    fn test_ebs_savings_gp2() {
        let savings = calculate_ebs_savings(100, "gp2");
        assert!((savings - 10.0).abs() < f64::EPSILON); // 100 * $0.10
    }

    #[test]
    fn test_ebs_savings_gp3() {
        let savings = calculate_ebs_savings(500, "gp3");
        assert!((savings - 40.0).abs() < f64::EPSILON); // 500 * $0.08
    }

    #[test]
    fn test_ebs_savings_io1() {
        let savings = calculate_ebs_savings(200, "io1");
        assert!((savings - 25.0).abs() < f64::EPSILON); // 200 * $0.125
    }

    #[test]
    fn test_ebs_savings_zero_size() {
        let savings = calculate_ebs_savings(0, "gp2");
        assert!((savings - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ebs_savings_unknown_type_uses_fallback() {
        let savings = calculate_ebs_savings(100, "unknown-type");
        assert!((savings - 10.0).abs() < f64::EPSILON); // 100 * $0.10 fallback
    }
}
