use tracing::{info, warn};

use super::{NewRecommendation, RuleEngine};

pub struct S3LifecycleRule;

impl RuleEngine for S3LifecycleRule {
    fn evaluate<'a>(
        &'a self,
        config: &'a aws_config::SdkConfig,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<Vec<NewRecommendation>>> + Send + 'a>> {
        Box::pin(async move {
        let s3 = aws_sdk_s3::Client::new(config);

        let region = config
            .region()
            .map(|r| r.to_string())
            .unwrap_or_else(|| "us-east-1".into());

        let mut recommendations = Vec::new();

        let resp = s3.list_buckets().send().await?;

        for bucket in resp.buckets() {
            let bucket_name = bucket.name().unwrap_or_default();
            if bucket_name.is_empty() {
                continue;
            }

            // Check if the bucket has lifecycle configuration
            let has_lifecycle = match s3
                .get_bucket_lifecycle_configuration()
                .bucket(bucket_name)
                .send()
                .await
            {
                Ok(lc_resp) => !lc_resp.rules().is_empty(),
                Err(e) => {
                    // NoSuchLifecycleConfiguration means no lifecycle rules
                    let is_no_lifecycle = e.to_string().contains("NoSuchLifecycleConfiguration")
                        || e.to_string().contains("The lifecycle configuration does not exist");
                    if !is_no_lifecycle {
                        warn!(bucket = bucket_name, error = %e, "Failed to check bucket lifecycle");
                    }
                    false
                }
            };

            if has_lifecycle {
                continue;
            }

            // Estimate storage size (head bucket doesn't return size, so we flag it generically)
            let terraform_code = format!(
                r#"# Add lifecycle rules to S3 bucket: {bucket_name}
resource "aws_s3_bucket_lifecycle_configuration" "{bucket_name}_lifecycle" {{
  bucket = "{bucket_name}"

  rule {{
    id     = "transition-to-ia"
    status = "Enabled"

    transition {{
      days          = 30
      storage_class = "STANDARD_IA"
    }}

    transition {{
      days          = 90
      storage_class = "GLACIER"
    }}

    transition {{
      days          = 180
      storage_class = "DEEP_ARCHIVE"
    }}

    noncurrent_version_transition {{
      noncurrent_days = 30
      storage_class   = "STANDARD_IA"
    }}

    noncurrent_version_expiration {{
      noncurrent_days = 90
    }}
  }}
}}"#
            );

            recommendations.push(NewRecommendation {
                rec_type: "cost_optimization".into(),
                provider: "aws".into(),
                resource_id: bucket_name.to_string(),
                resource_type: "S3 Bucket".into(),
                region: region.clone(),
                account_id: String::new(),
                estimated_savings: 0.0, // Can't estimate without knowing bucket size
                estimated_savings_pct: 0.0,
                current_config: serde_json::json!({
                    "bucket_name": bucket_name,
                    "lifecycle_rules": false,
                    "note": "No lifecycle rules configured. Objects remain in STANDARD storage class indefinitely.",
                }),
                recommended_config: serde_json::json!({
                    "action": "add_lifecycle_rules",
                    "reason": "No lifecycle rules configured. Adding transitions to S3-IA (30 days), Glacier (90 days), and Deep Archive (180 days) can reduce storage costs by up to 70%.",
                    "transitions": [
                        {"days": 30, "class": "STANDARD_IA", "savings": "~40% vs Standard"},
                        {"days": 90, "class": "GLACIER", "savings": "~68% vs Standard"},
                        {"days": 180, "class": "DEEP_ARCHIVE", "savings": "~95% vs Standard"},
                    ],
                }),
                impact: "medium".into(),
                effort: "low".into(),
                risk: "low".into(),
                rule_id: "s3-lifecycle".into(),
                severity: "medium".into(),
                details: serde_json::json!({}),
                terraform_code: Some(terraform_code),
            });
        }

        info!(count = recommendations.len(), "S3 lifecycle rule completed");
        Ok(recommendations)
        })
    }
}
