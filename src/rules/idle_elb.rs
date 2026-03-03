use tracing::{info, warn};

use super::{get_metric_sum, NewRecommendation, RuleEngine};

/// Approximate hourly cost for ALB/NLB.
fn elb_hourly_cost(elb_type: &str) -> f64 {
    match elb_type {
        "application" => 0.0225,
        "network" => 0.0225,
        "gateway" => 0.0125,
        _ => 0.0225,
    }
}

const HOURS_PER_MONTH: f64 = 730.0;
const REQUEST_THRESHOLD_PER_DAY: f64 = 100.0;
const LOOKBACK_DAYS: i64 = 7;

pub struct IdleElbRule;

impl RuleEngine for IdleElbRule {
    fn evaluate<'a>(
        &'a self,
        config: &'a aws_config::SdkConfig,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<Vec<NewRecommendation>>> + Send + 'a>> {
        Box::pin(async move {
        let elbv2 = aws_sdk_elasticloadbalancingv2::Client::new(config);
        let cw = aws_sdk_cloudwatch::Client::new(config);

        let region = config
            .region()
            .map(|r| r.to_string())
            .unwrap_or_else(|| "us-east-1".into());

        let mut recommendations = Vec::new();

        let resp = elbv2.describe_load_balancers().send().await?;

        for lb in resp.load_balancers() {
            let lb_arn = lb.load_balancer_arn().unwrap_or_default();
            let lb_name = lb.load_balancer_name().unwrap_or_default();
            let lb_type = lb.r#type()
                .map(|t| t.as_str().to_string())
                .unwrap_or_else(|| "application".into());

            if lb_arn.is_empty() {
                continue;
            }

            // Check target group health
            let tg_resp = elbv2
                .describe_target_groups()
                .load_balancer_arn(lb_arn)
                .send()
                .await;

            let mut has_healthy_targets = false;
            if let Ok(tg_resp) = &tg_resp {
                for tg in tg_resp.target_groups() {
                    let tg_arn = tg.target_group_arn().unwrap_or_default();
                    if tg_arn.is_empty() { continue; }

                    if let Ok(health) = elbv2
                        .describe_target_health()
                        .target_group_arn(tg_arn)
                        .send()
                        .await
                    {
                        for desc in health.target_health_descriptions() {
                            if let Some(th) = desc.target_health() {
                                if th.state() == Some(&aws_sdk_elasticloadbalancingv2::types::TargetHealthStateEnum::Healthy) {
                                    has_healthy_targets = true;
                                    break;
                                }
                            }
                        }
                    }
                    if has_healthy_targets { break; }
                }
            }

            // Extract the ALB suffix for CloudWatch dimension: app/my-lb/50dc6c495c0c9188
            let arn_suffix = lb_arn
                .split("loadbalancer/")
                .nth(1)
                .unwrap_or(lb_name);

            let total_requests = match get_metric_sum(
                &cw,
                "AWS/ApplicationELB",
                "RequestCount",
                "LoadBalancer",
                arn_suffix,
                LOOKBACK_DAYS,
            )
            .await
            {
                Ok(v) => v,
                Err(e) => {
                    warn!(lb_name, error = %e, "Failed to get ELB request metrics");
                    0.0
                }
            };

            let avg_daily_requests = total_requests / LOOKBACK_DAYS as f64;
            let is_idle = !has_healthy_targets || avg_daily_requests < REQUEST_THRESHOLD_PER_DAY;

            if !is_idle {
                continue;
            }

            let monthly_savings = elb_hourly_cost(&lb_type) * HOURS_PER_MONTH;

            let reason = if !has_healthy_targets {
                "Load balancer has zero healthy targets".to_string()
            } else {
                format!("Average {avg_daily_requests:.0} requests/day < {REQUEST_THRESHOLD_PER_DAY} threshold over {LOOKBACK_DAYS} days")
            };

            let terraform_code = format!(
                r#"# Remove idle load balancer: {lb_name}
# WARNING: Ensure DNS records and dependent services are updated before destroying
resource "aws_lb" "{lb_name}" {{
  # This resource should be removed from your Terraform state
  # Run: terraform state rm aws_lb.{lb_name}
}}

# If managed by Terraform, simply remove the resource block and run terraform apply
# Otherwise use AWS CLI:
# aws elbv2 delete-load-balancer --load-balancer-arn {lb_arn}"#
            );

            recommendations.push(NewRecommendation {
                rec_type: "unused_resource".into(),
                provider: "aws".into(),
                resource_id: lb_arn.to_string(),
                resource_type: "Load Balancer".into(),
                region: region.clone(),
                account_id: String::new(),
                estimated_savings: monthly_savings,
                estimated_savings_pct: 100.0,
                current_config: serde_json::json!({
                    "name": lb_name,
                    "type": lb_type,
                    "has_healthy_targets": has_healthy_targets,
                    "avg_daily_requests": format!("{avg_daily_requests:.0}"),
                }),
                recommended_config: serde_json::json!({
                    "action": "delete",
                    "reason": reason,
                }),
                impact: "medium".into(),
                effort: "medium".into(),
                risk: "medium".into(),
                rule_id: "idle-elb".into(),
                severity: if monthly_savings > 50.0 { "medium".into() } else { "low".into() },
                details: serde_json::json!({}),
                terraform_code: Some(terraform_code),
            });
        }

        info!(count = recommendations.len(), "Idle ELB rule completed");
        Ok(recommendations)
        })
    }
}
