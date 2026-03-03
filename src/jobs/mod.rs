pub mod recommendations;

use sqlx::PgPool;
use tokio::time::{interval, Duration};
use tracing::{info, error};
use uuid::Uuid;
use chrono::{Utc, NaiveDate};

use crate::config::{JobsConfig, LlmConfig};
use crate::db::{CostRepo, BudgetRepo, AnomalyRepo, ForecastRepo, CloudProviderRepo};
use crate::ml;
use crate::models::{Anomaly, Forecast, ForecastPoint, CostRecord, AwsCredentials, AzureCredentials};
use crate::ws::WsHub;

pub fn spawn_background_jobs(
    pool: PgPool,
    config: JobsConfig,
    ws_hub: WsHub,
    encryption_key: String,
    llm_config: LlmConfig,
) {
    // Cost sync job
    let pool0 = pool.clone();
    let ws0 = ws_hub.clone();
    let enc_key = encryption_key.clone();
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(config.cost_sync_interval_secs));
        loop {
            ticker.tick().await;
            info!("Running cost sync job");
            if let Err(e) = run_cost_sync(&pool0, &ws0, &enc_key).await {
                error!("Cost sync job failed: {e}");
            }
        }
    });

    let pool1 = pool.clone();
    let ws1 = ws_hub.clone();
    let llm1 = llm_config;
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(config.anomaly_detect_interval_secs));
        loop {
            ticker.tick().await;
            info!("Running anomaly detection job");
            if let Err(e) = run_anomaly_detection(&pool1, &ws1, &llm1).await {
                error!("Anomaly detection job failed: {e}");
            }
        }
    });

    let pool2 = pool.clone();
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(config.forecast_interval_secs));
        loop {
            ticker.tick().await;
            info!("Running forecast job");
            if let Err(e) = run_forecast(&pool2).await {
                error!("Forecast job failed: {e}");
            }
        }
    });

    let pool3 = pool.clone();
    let ws3 = ws_hub.clone();
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(config.budget_check_interval_secs));
        loop {
            ticker.tick().await;
            info!("Running budget check job");
            if let Err(e) = run_budget_check(&pool3, &ws3).await {
                error!("Budget check job failed: {e}");
            }
        }
    });

    // Recommendation rules engine job
    let pool4 = pool.clone();
    let enc_key4 = encryption_key.clone();
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(config.recommendation_interval_secs));
        loop {
            ticker.tick().await;
            info!("Running recommendation rules engine");
            match recommendations::run_recommendation_scan(&pool4, &enc_key4).await {
                Ok(count) => info!(count, "Recommendation scan completed"),
                Err(e) => error!("Recommendation scan failed: {e}"),
            }
        }
    });

    info!("Background jobs started");
}

/// Sync cost data from all enabled cloud providers.
async fn run_cost_sync(pool: &PgPool, ws_hub: &WsHub, encryption_key: &str) -> anyhow::Result<()> {
    let providers = CloudProviderRepo::get_all_enabled(pool).await?;
    let end_date = Utc::now().date_naive();
    let start_date = end_date - chrono::Duration::days(7); // Sync last 7 days

    for provider in providers {
        if provider.status == "credentials_error" {
            continue;
        }

        let creds_enc = match &provider.credentials {
            Some(c) => c,
            None => continue,
        };

        let creds_bytes = match crate::crypto::decrypt(creds_enc, encryption_key) {
            Ok(b) => b,
            Err(e) => {
                error!(provider_id = %provider.id, "Failed to decrypt credentials: {e}");
                CloudProviderRepo::update_status(
                    pool,
                    provider.id,
                    "credentials_error",
                    Some("Decryption failed — re-save credentials via PUT /api/v1/providers/:id"),
                ).await.ok();
                continue;
            }
        };

        let creds_json: serde_json::Value = match serde_json::from_slice(&creds_bytes) {
            Ok(v) => v,
            Err(e) => {
                error!(provider_id = %provider.id, "Failed to parse credentials: {e}");
                continue;
            }
        };

        let cost_items = match provider.provider_type.as_str() {
            "aws" => {
                let aws_creds: AwsCredentials = match serde_json::from_value(creds_json) {
                    Ok(c) => c,
                    Err(e) => {
                        error!(provider_id = %provider.id, "Invalid AWS creds: {e}");
                        continue;
                    }
                };
                let account = aws_creds.access_key_id.clone();
                match crate::cloud::aws::sync_costs(&aws_creds, start_date, end_date, &account).await {
                    Ok(items) => items,
                    Err(e) => {
                        error!(provider_id = %provider.id, "AWS sync error: {e}");
                        CloudProviderRepo::update_status(pool, provider.id, "failed", Some(&e.to_string())).await.ok();
                        continue;
                    }
                }
            }
            "azure" => {
                let azure_creds: AzureCredentials = match serde_json::from_value(creds_json) {
                    Ok(c) => c,
                    Err(e) => {
                        error!(provider_id = %provider.id, "Invalid Azure creds: {e}");
                        continue;
                    }
                };
                match crate::cloud::azure::sync_costs(&azure_creds, start_date, end_date).await {
                    Ok(items) => items,
                    Err(e) => {
                        error!(provider_id = %provider.id, "Azure sync error: {e}");
                        CloudProviderRepo::update_status(pool, provider.id, "failed", Some(&e.to_string())).await.ok();
                        continue;
                    }
                }
            }
            _ => continue,
        };

        let records: Vec<CostRecord> = cost_items
            .into_iter()
            .map(|item| CostRecord {
                id: Uuid::new_v4(),
                organization_id: provider.organization_id,
                date: item.date,
                amount: rust_decimal::Decimal::from_f64_retain(item.amount).unwrap_or_default(),
                currency: item.currency,
                provider: provider.provider_type.clone(),
                service: item.service,
                account_id: item.account_id,
                region: item.region,
                resource_id: item.resource_id,
                tags: item.tags,
                estimated: item.estimated,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .collect();

        let count = records.len();
        if !records.is_empty() {
            CostRepo::create_batch(pool, &records).await?;
            CloudProviderRepo::update_sync_time(pool, provider.id).await?;
            info!(provider_id = %provider.id, count, "Synced cost data");

            ws_hub.send_cost_update(provider.organization_id, serde_json::json!({
                "provider": provider.provider_type,
                "records_synced": count,
            })).await;
        }
    }

    Ok(())
}

async fn run_anomaly_detection(pool: &PgPool, ws_hub: &WsHub, llm_config: &LlmConfig) -> anyhow::Result<()> {
    let orgs = crate::db::OrgRepo::list(pool).await?;

    for org in orgs {
        let end = Utc::now().date_naive();
        let start = end - chrono::Duration::days(30);

        let daily_totals = CostRepo::get_daily_totals(pool, org.id, start, end).await?;
        if daily_totals.len() < 15 {
            continue;
        }

        let values: Vec<f64> = daily_totals.iter().map(|(_, v)| *v).collect();
        let dates: Vec<NaiveDate> = daily_totals.iter().map(|(d, _)| *d).collect();

        let detector = ml::anomaly::AnomalyDetector::new(0.1);
        let detected = detector.detect(&values);

        let mut new_anomalies = Vec::new();
        for a in &detected {
            let anomaly = Anomaly {
                id: Uuid::new_v4(),
                organization_id: org.id,
                date: dates[a.index],
                actual_amount: rust_decimal::Decimal::from_f64_retain(a.value).unwrap_or_default(),
                expected_amount: rust_decimal::Decimal::from_f64_retain(a.expected).unwrap_or_default(),
                deviation: rust_decimal::Decimal::from_f64_retain(a.deviation).unwrap_or_default(),
                deviation_pct: rust_decimal::Decimal::from_f64_retain(a.deviation_pct).unwrap_or_default(),
                score: rust_decimal::Decimal::from_f64_retain(a.score).unwrap_or_default(),
                severity: a.severity.clone(),
                status: "open".into(),
                provider: String::new(),
                service: String::new(),
                account_id: String::new(),
                region: String::new(),
                root_cause: None,
                notes: None,
                detected_at: Utc::now(),
                acknowledged_at: None,
                acknowledged_by: None,
                resolved_at: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            new_anomalies.push(anomaly);
        }

        if !new_anomalies.is_empty() {
            AnomalyRepo::create_batch(pool, &new_anomalies).await?;
            info!(org_id = %org.id, count = new_anomalies.len(), "Detected anomalies");

            // LLM root cause analysis for HIGH and CRITICAL anomalies (max 5 per run)
            let mut llm_calls = 0;
            const MAX_LLM_CALLS: usize = 5;

            // Gather context for LLM: 7-day cost trend and service breakdown
            let trend_start = end - chrono::Duration::days(7);
            let trend_data = CostRepo::get_daily_totals(pool, org.id, trend_start, end).await.unwrap_or_default();
            let trend_summary: String = trend_data.iter()
                .map(|(d, v)| format!("  {d}: ${v:.2}"))
                .collect::<Vec<_>>()
                .join("\n");

            let breakdown = CostRepo::get_breakdown(pool, org.id, trend_start, end, "service").await.ok();
            let service_summary = breakdown.map(|b| {
                b.items.iter().take(10)
                    .map(|i| format!("  - {}: ${:.2} ({:.1}%)", i.name, i.amount, i.percentage))
                    .collect::<Vec<_>>()
                    .join("\n")
            }).unwrap_or_else(|| "Service breakdown unavailable.".into());

            for a in &new_anomalies {
                if llm_calls >= MAX_LLM_CALLS {
                    break;
                }

                if a.severity != "high" && a.severity != "critical" {
                    continue;
                }

                llm_calls += 1;

                let system_prompt = "You are a FinOps cloud cost analyst. Analyze the anomaly data and provide a concise root cause analysis. \
                    Focus on likely causes based on the spending pattern, affected date, deviation amount, and service breakdown. \
                    Be specific and actionable. Keep your response under 200 words.";

                let user_message = format!(
                    "Anomaly detected on {date}:\n\
                     - Actual spend: ${actual}\n\
                     - Expected spend: ${expected}\n\
                     - Deviation: {deviation_pct}%\n\
                     - Severity: {severity}\n\n\
                     7-day cost trend:\n{trend}\n\n\
                     Service breakdown (last 7 days):\n{services}\n\n\
                     What is the most likely root cause of this cost anomaly?",
                    date = a.date,
                    actual = a.actual_amount,
                    expected = a.expected_amount,
                    deviation_pct = a.deviation_pct,
                    severity = a.severity,
                    trend = trend_summary,
                    services = service_summary,
                );

                match crate::llm::call_llm(llm_config, system_prompt, &user_message).await {
                    Ok(root_cause) => {
                        // Update the anomaly with the root cause
                        let update_req = crate::models::UpdateAnomalyRequest {
                            status: None,
                            notes: None,
                            root_cause: Some(root_cause.clone()),
                        };
                        if let Err(e) = AnomalyRepo::update(pool, org.id, a.id, &update_req).await {
                            error!(anomaly_id = %a.id, error = %e, "Failed to update anomaly root cause");
                        } else {
                            info!(anomaly_id = %a.id, "LLM root cause analysis stored");
                        }
                    }
                    Err(e) => {
                        error!(anomaly_id = %a.id, error = %e, "LLM root cause analysis failed");
                        // Don't fail the job, just log and continue
                    }
                }

                // Send WebSocket alert (after potential LLM enrichment)
                ws_hub.send_anomaly_alert(org.id, serde_json::json!({
                    "severity": a.severity,
                    "date": a.date.to_string(),
                    "actual": a.actual_amount.to_string(),
                    "expected": a.expected_amount.to_string(),
                })).await;
            }

            // Send WebSocket alerts for remaining high/critical anomalies not processed by LLM
            for a in &new_anomalies {
                if (a.severity == "high" || a.severity == "critical") && llm_calls <= MAX_LLM_CALLS {
                    // Already handled above
                    continue;
                }
                if a.severity == "high" || a.severity == "critical" {
                    ws_hub.send_anomaly_alert(org.id, serde_json::json!({
                        "severity": a.severity,
                        "date": a.date.to_string(),
                        "actual": a.actual_amount.to_string(),
                        "expected": a.expected_amount.to_string(),
                    })).await;
                }
            }
        }
    }

    Ok(())
}

async fn run_forecast(pool: &PgPool) -> anyhow::Result<()> {
    let orgs = crate::db::OrgRepo::list(pool).await?;

    for org in orgs {
        let end = Utc::now().date_naive();
        let start = end - chrono::Duration::days(90);

        let daily_totals = CostRepo::get_daily_totals(pool, org.id, start, end).await?;
        if daily_totals.len() < 14 {
            continue;
        }

        let values: Vec<f64> = daily_totals.iter().map(|(_, v)| *v).collect();

        match ml::forecast::generate_forecast(&values, 30) {
            Ok(result) => {
                let predictions: Vec<ForecastPoint> = (0..result.predicted.len())
                    .map(|i| {
                        let date = end + chrono::Duration::days(i as i64 + 1);
                        ForecastPoint {
                            date,
                            predicted: result.predicted[i],
                            lower_bound: result.lower[i],
                            upper_bound: result.upper[i],
                        }
                    })
                    .collect();

                let total: f64 = result.predicted.iter().sum();

                let forecast = Forecast {
                    id: Uuid::new_v4(),
                    organization_id: org.id,
                    generated_at: Utc::now(),
                    model_version: "augurs-ets-1.0".into(),
                    granularity: "daily".into(),
                    predictions: serde_json::to_value(&predictions).unwrap_or_default(),
                    total_forecasted: rust_decimal::Decimal::from_f64_retain(total).unwrap_or_default(),
                    confidence_level: rust_decimal::Decimal::from_f64_retain(result.confidence).unwrap_or_default(),
                    currency: "USD".into(),
                    service_filter: None,
                    account_filter: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };

                ForecastRepo::create(pool, &forecast).await?;
                info!(org_id = %org.id, total_forecasted = total, "Generated forecast");
            }
            Err(e) => {
                error!(org_id = %org.id, error = %e, "Forecast generation failed");
            }
        }
    }

    Ok(())
}

async fn run_budget_check(pool: &PgPool, ws_hub: &WsHub) -> anyhow::Result<()> {
    let budgets = BudgetRepo::get_all(pool).await?;
    let now = Utc::now().date_naive();

    for budget in budgets {
        let (period_start, period_end) = match budget.period.as_str() {
            "monthly" => {
                let start = NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap_or(now);
                let end = if now.month() == 12 {
                    NaiveDate::from_ymd_opt(now.year() + 1, 1, 1).unwrap_or(now)
                } else {
                    NaiveDate::from_ymd_opt(now.year(), now.month() + 1, 1).unwrap_or(now)
                } - chrono::Duration::days(1);
                (start, end)
            }
            "quarterly" => {
                let q = ((now.month() - 1) / 3) * 3 + 1;
                let start = NaiveDate::from_ymd_opt(now.year(), q, 1).unwrap_or(now);
                let end_month = q + 3;
                let end = if end_month > 12 {
                    NaiveDate::from_ymd_opt(now.year() + 1, end_month - 12, 1).unwrap_or(now)
                } else {
                    NaiveDate::from_ymd_opt(now.year(), end_month, 1).unwrap_or(now)
                } - chrono::Duration::days(1);
                (start, end)
            }
            _ => {
                let start = NaiveDate::from_ymd_opt(now.year(), 1, 1).unwrap_or(now);
                let end = NaiveDate::from_ymd_opt(now.year(), 12, 31).unwrap_or(now);
                (start, end)
            }
        };

        let current_spend = CostRepo::get_period_total(pool, budget.organization_id, period_start, period_end).await?;

        let budget_amount: f64 = budget.amount.try_into().unwrap_or(0.0);
        let spend_pct = if budget_amount > 0.0 { (current_spend / budget_amount) * 100.0 } else { 0.0 };

        let status = if spend_pct >= 100.0 {
            "exceeded"
        } else if spend_pct >= 80.0 {
            "warning"
        } else {
            "active"
        };

        BudgetRepo::update_spend(pool, budget.id, current_spend, status).await?;

        if status == "exceeded" || status == "warning" {
            ws_hub.send_budget_alert(budget.organization_id, serde_json::json!({
                "budget_name": budget.name,
                "budget_amount": budget_amount,
                "current_spend": current_spend,
                "spend_pct": spend_pct,
                "status": status,
            })).await;
        }
    }

    Ok(())
}

use chrono::Datelike;
