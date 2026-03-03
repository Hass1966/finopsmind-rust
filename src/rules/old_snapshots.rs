use chrono::{DateTime, Utc};
use tracing::info;

use super::{NewRecommendation, RuleEngine};

const SNAPSHOT_PRICE_PER_GB: f64 = 0.05;
const MAX_AGE_DAYS: i64 = 90;

pub struct OldSnapshotsRule;

impl RuleEngine for OldSnapshotsRule {
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
        let cutoff = Utc::now() - chrono::Duration::days(MAX_AGE_DAYS);

        // Paginate through all snapshots owned by this account
        let mut next_token: Option<String> = None;
        loop {
            let mut req = ec2
                .describe_snapshots()
                .owner_ids("self");

            if let Some(token) = &next_token {
                req = req.next_token(token);
            }

            let resp = req.send().await?;

            for snap in resp.snapshots() {
                let snap_id = snap.snapshot_id().unwrap_or_default();
                let size_gb = snap.volume_size().unwrap_or(0);

                if snap_id.is_empty() || size_gb == 0 {
                    continue;
                }

                let start_time = match snap.start_time() {
                    Some(t) => t,
                    None => continue,
                };

                let created_at = match DateTime::from_timestamp(
                    start_time.secs(),
                    start_time.subsec_nanos(),
                ) {
                    Some(dt) => dt,
                    None => continue,
                };

                if created_at >= cutoff {
                    continue; // not old enough
                }

                let age_days = (Utc::now() - created_at).num_days();
                let monthly_savings = size_gb as f64 * SNAPSHOT_PRICE_PER_GB;

                let description = snap.description().unwrap_or("").to_string();
                let volume_id = snap.volume_id().unwrap_or("").to_string();

                let name = snap
                    .tags()
                    .iter()
                    .find(|t| t.key() == Some("Name"))
                    .and_then(|t| t.value())
                    .unwrap_or("")
                    .to_string();

                recommendations.push(NewRecommendation {
                    rec_type: "unused_resource".into(),
                    provider: "aws".into(),
                    resource_id: snap_id.to_string(),
                    resource_type: "EBS Snapshot".into(),
                    region: region.clone(),
                    account_id: String::new(),
                    estimated_savings: monthly_savings,
                    estimated_savings_pct: 100.0,
                    current_config: serde_json::json!({
                        "size_gb": size_gb,
                        "age_days": age_days,
                        "created_at": created_at.to_rfc3339(),
                        "description": description,
                        "source_volume": volume_id,
                        "name": name,
                    }),
                    recommended_config: serde_json::json!({
                        "action": "delete",
                        "reason": format!("Snapshot is {age_days} days old (threshold: {MAX_AGE_DAYS} days)"),
                    }),
                    impact: "low".into(),
                    effort: "low".into(),
                    risk: "low".into(),
                    rule_id: "old-snapshots".into(),
                    severity: "low".into(),
                    details: serde_json::json!({}),
                });
            }

            next_token = resp.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        info!(count = recommendations.len(), "Old snapshots rule completed");
        Ok(recommendations)
        })
    }
}

/// Calculate monthly savings from deleting a snapshot.
#[cfg(test)]
fn snapshot_savings(size_gb: i32) -> f64 {
    size_gb as f64 * SNAPSHOT_PRICE_PER_GB
}

/// Determine whether a snapshot's creation timestamp is older than the threshold.
#[cfg(test)]
fn is_snapshot_old(created_at: &DateTime<Utc>, max_age_days: i64) -> bool {
    let cutoff = Utc::now() - chrono::Duration::days(max_age_days);
    *created_at < cutoff
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_savings_basic() {
        let savings = snapshot_savings(100);
        assert!((savings - 5.0).abs() < f64::EPSILON); // 100 * $0.05
    }

    #[test]
    fn test_snapshot_savings_large() {
        let savings = snapshot_savings(1000);
        assert!((savings - 50.0).abs() < f64::EPSILON); // 1000 * $0.05
    }

    #[test]
    fn test_snapshot_savings_zero() {
        let savings = snapshot_savings(0);
        assert!((savings - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_old_snapshot_91_days() {
        let created = Utc::now() - chrono::Duration::days(91);
        assert!(is_snapshot_old(&created, 90));
    }

    #[test]
    fn test_old_snapshot_exactly_90_days() {
        // Exactly 90 days ago should be slightly past cutoff due to sub-second precision
        let created = Utc::now() - chrono::Duration::days(90) - chrono::Duration::seconds(1);
        assert!(is_snapshot_old(&created, 90));
    }

    #[test]
    fn test_recent_snapshot_89_days() {
        let created = Utc::now() - chrono::Duration::days(89);
        assert!(!is_snapshot_old(&created, 90));
    }

    #[test]
    fn test_very_old_snapshot() {
        let created = Utc::now() - chrono::Duration::days(365);
        assert!(is_snapshot_old(&created, 90));
    }

    #[test]
    fn test_just_created_snapshot() {
        let created = Utc::now();
        assert!(!is_snapshot_old(&created, 90));
    }

    #[test]
    fn test_custom_threshold() {
        let created = Utc::now() - chrono::Duration::days(31);
        assert!(is_snapshot_old(&created, 30));
        assert!(!is_snapshot_old(&created, 60));
    }
}
