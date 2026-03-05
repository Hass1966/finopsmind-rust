use chrono::Utc;
use sqlx::PgPool;
use tracing::{error, info};
use uuid::Uuid;

use crate::db::{CloudProviderRepo, RecommendationRepo};
use crate::models::{AwsCredentials, Recommendation};
use crate::rules::{
    build_aws_config,
    idle_ec2::IdleEc2Rule,
    old_snapshots::OldSnapshotsRule,
    oversized_rds::OversizedRdsRule,
    unattached_ebs::UnattachedEbsRule,
    idle_elb::IdleElbRule,
    missing_ri::MissingRiRule,
    s3_lifecycle::S3LifecycleRule,
    idle_eip::IdleEipRule,
    serverless_migration::ServerlessMigrationRule,
    NewRecommendation,
    RuleEngine,
};

/// Run all recommendation rules for all enabled AWS providers and persist results.
/// Returns the total count of new recommendations created.
pub async fn run_recommendation_scan(pool: &PgPool, encryption_key: &str) -> anyhow::Result<usize> {
    let providers = CloudProviderRepo::get_all_enabled(pool).await?;
    let mut total_count = 0;

    for provider in providers {
        if provider.provider_type != "aws" {
            continue; // rules engine currently only supports AWS
        }

        let creds_enc = match &provider.credentials {
            Some(c) => c,
            None => continue,
        };

        let creds_bytes = match crate::crypto::decrypt(creds_enc, encryption_key) {
            Ok(b) => b,
            Err(e) => {
                error!(provider_id = %provider.id, "Failed to decrypt credentials: {e}");
                continue;
            }
        };

        let aws_creds: AwsCredentials = match serde_json::from_slice(&creds_bytes) {
            Ok(c) => c,
            Err(e) => {
                error!(provider_id = %provider.id, "Invalid AWS credentials: {e}");
                continue;
            }
        };

        let sdk_config = build_aws_config(&aws_creds);
        let account_id = aws_creds.access_key_id.clone();

        let rules: Vec<(&str, Box<dyn RuleEngine>)> = vec![
            ("idle-ec2", Box::new(IdleEc2Rule)),
            ("oversized-rds", Box::new(OversizedRdsRule)),
            ("unattached-ebs", Box::new(UnattachedEbsRule)),
            ("old-snapshots", Box::new(OldSnapshotsRule)),
            ("idle-elb", Box::new(IdleElbRule)),
            ("missing-ri", Box::new(MissingRiRule)),
            ("s3-lifecycle", Box::new(S3LifecycleRule)),
            ("idle-eip", Box::new(IdleEipRule)),
            ("serverless-migration", Box::new(ServerlessMigrationRule)),
        ];

        let mut all_new: Vec<NewRecommendation> = Vec::new();

        for (rule_name, rule) in &rules {
            match rule.evaluate(&sdk_config).await {
                Ok(mut recs) => {
                    // Fill in account_id from provider credentials
                    for rec in &mut recs {
                        if rec.account_id.is_empty() {
                            rec.account_id = account_id.clone();
                        }
                    }
                    info!(
                        provider_id = %provider.id,
                        rule = rule_name,
                        count = recs.len(),
                        "Rule evaluation completed"
                    );
                    all_new.extend(recs);
                }
                Err(e) => {
                    error!(
                        provider_id = %provider.id,
                        rule = rule_name,
                        error = %e,
                        "Rule evaluation failed"
                    );
                }
            }
        }

        if !all_new.is_empty() {
            let records: Vec<Recommendation> = all_new
                .into_iter()
                .map(|nr| new_to_recommendation(nr, provider.organization_id))
                .collect();

            let count = records.len();
            for rec in &records {
                if let Err(e) = RecommendationRepo::create(pool, rec).await {
                    error!(
                        resource_id = %rec.resource_id,
                        error = %e,
                        "Failed to insert recommendation"
                    );
                }
            }
            total_count += count;
            info!(
                provider_id = %provider.id,
                count,
                "Persisted recommendations"
            );
        }
    }

    info!(total = total_count, "Recommendation scan completed");
    Ok(total_count)
}

fn new_to_recommendation(nr: NewRecommendation, org_id: Uuid) -> Recommendation {
    Recommendation {
        id: Uuid::new_v4(),
        organization_id: org_id,
        rec_type: nr.rec_type,
        provider: nr.provider,
        account_id: nr.account_id,
        region: nr.region,
        resource_id: nr.resource_id,
        resource_type: nr.resource_type,
        current_config: nr.current_config,
        recommended_config: nr.recommended_config,
        estimated_savings: rust_decimal::Decimal::from_f64_retain(nr.estimated_savings)
            .unwrap_or_default(),
        estimated_savings_pct: rust_decimal::Decimal::from_f64_retain(nr.estimated_savings_pct)
            .unwrap_or_default(),
        currency: "USD".into(),
        impact: nr.impact,
        effort: nr.effort,
        risk: nr.risk,
        status: "pending".into(),
        details: nr.details,
        notes: None,
        implemented_by: None,
        implemented_at: None,
        rule_id: Some(nr.rule_id),
        confidence: Some("high".into()),
        terraform_code: nr.terraform_code,
        resource_metadata: serde_json::json!({}),
        resource_arn: None,
        expires_at: None,
        severity: Some(nr.severity),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}
