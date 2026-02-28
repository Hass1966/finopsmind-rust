use std::sync::Arc;
use sqlx::PgPool;
use tokio::time::{interval, Duration};
use tracing::{info, error};
use uuid::Uuid;
use chrono::{Utc, NaiveDate};

use crate::config::JobsConfig;
use crate::db::{CostRepo, BudgetRepo, AnomalyRepo, ForecastRepo};
use crate::ml;
use crate::models::{Anomaly, Forecast, ForecastPoint};
use crate::ws::WsHub;

pub fn spawn_background_jobs(
    pool: PgPool,
    config: JobsConfig,
    ws_hub: WsHub,
) {
    let pool1 = pool.clone();
    let ws1 = ws_hub.clone();
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(config.anomaly_detect_interval_secs));
        loop {
            ticker.tick().await;
            info!("Running anomaly detection job");
            if let Err(e) = run_anomaly_detection(&pool1, &ws1).await {
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

    info!("Background jobs started");
}

async fn run_anomaly_detection(pool: &PgPool, ws_hub: &WsHub) -> anyhow::Result<()> {
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

            for a in &new_anomalies {
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
        // Determine period dates
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
