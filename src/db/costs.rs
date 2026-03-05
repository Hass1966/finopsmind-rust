use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{AiCostSummary, CostBreakdown, CostBreakdownItem, CostDataPoint, CostRecord, CostSummary, CostTrend};

pub struct CostRepo;

impl CostRepo {
    #[allow(dead_code)]
    pub async fn create(pool: &PgPool, record: &CostRecord) -> Result<CostRecord, sqlx::Error> {
        sqlx::query_as::<_, CostRecord>(
            r#"INSERT INTO costs (id, organization_id, date, amount, currency, provider, service, account_id, region, resource_id, tags, estimated)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
               ON CONFLICT (organization_id, date, provider, service, account_id, region, resource_id)
               DO UPDATE SET amount = EXCLUDED.amount, tags = EXCLUDED.tags, estimated = EXCLUDED.estimated, updated_at = NOW()
               RETURNING *"#,
        )
        .bind(record.id)
        .bind(record.organization_id)
        .bind(record.date)
        .bind(record.amount)
        .bind(&record.currency)
        .bind(&record.provider)
        .bind(&record.service)
        .bind(&record.account_id)
        .bind(&record.region)
        .bind(&record.resource_id)
        .bind(&record.tags)
        .bind(record.estimated)
        .fetch_one(pool)
        .await
    }

    pub async fn create_batch(pool: &PgPool, records: &[CostRecord]) -> Result<(), sqlx::Error> {
        let mut tx = pool.begin().await?;
        for record in records {
            sqlx::query(
                r#"INSERT INTO costs (id, organization_id, date, amount, currency, provider, service, account_id, region, resource_id, tags, estimated)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                   ON CONFLICT (organization_id, date, provider, service, account_id, region, resource_id)
                   DO UPDATE SET amount = EXCLUDED.amount, tags = EXCLUDED.tags, estimated = EXCLUDED.estimated, updated_at = NOW()"#,
            )
            .bind(record.id)
            .bind(record.organization_id)
            .bind(record.date)
            .bind(record.amount)
            .bind(&record.currency)
            .bind(&record.provider)
            .bind(&record.service)
            .bind(&record.account_id)
            .bind(&record.region)
            .bind(&record.resource_id)
            .bind(&record.tags)
            .bind(record.estimated)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_summary(
        pool: &PgPool,
        org_id: Uuid,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<CostSummary, sqlx::Error> {
        // Current period total
        let total: (Option<f64>,) = sqlx::query_as(
            "SELECT COALESCE(SUM(amount::float8), 0) FROM costs WHERE organization_id = $1 AND date >= $2 AND date <= $3"
        )
        .bind(org_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_one(pool)
        .await?;

        // By service breakdown
        let rows: Vec<(String, f64)> = sqlx::query_as(
            "SELECT service, COALESCE(SUM(amount::float8), 0) as total FROM costs WHERE organization_id = $1 AND date >= $2 AND date <= $3 GROUP BY service ORDER BY total DESC"
        )
        .bind(org_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(pool)
        .await?;

        let total_cost = total.0.unwrap_or(0.0);
        let by_service: Vec<CostBreakdownItem> = rows
            .into_iter()
            .map(|(name, amount)| CostBreakdownItem {
                name,
                amount,
                percentage: if total_cost > 0.0 { (amount / total_cost) * 100.0 } else { 0.0 },
            })
            .collect();

        // By provider breakdown
        let provider_rows: Vec<(String, f64)> = sqlx::query_as(
            "SELECT provider, COALESCE(SUM(amount::float8), 0) as total FROM costs WHERE organization_id = $1 AND date >= $2 AND date <= $3 GROUP BY provider ORDER BY total DESC"
        )
        .bind(org_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(pool)
        .await?;

        let by_provider: Vec<CostBreakdownItem> = provider_rows
            .into_iter()
            .map(|(name, amount)| CostBreakdownItem {
                name,
                amount,
                percentage: if total_cost > 0.0 { (amount / total_cost) * 100.0 } else { 0.0 },
            })
            .collect();

        // Previous period for comparison
        let days = (end_date - start_date).num_days();
        let prev_start = start_date - chrono::Duration::days(days);
        let prev_end = start_date - chrono::Duration::days(1);

        let prev_total: (Option<f64>,) = sqlx::query_as(
            "SELECT COALESCE(SUM(amount::float8), 0) FROM costs WHERE organization_id = $1 AND date >= $2 AND date <= $3"
        )
        .bind(org_id)
        .bind(prev_start)
        .bind(prev_end)
        .fetch_one(pool)
        .await?;

        let prev_cost = prev_total.0.unwrap_or(0.0);
        let change_pct = if prev_cost > 0.0 {
            Some(((total_cost - prev_cost) / prev_cost) * 100.0)
        } else {
            None
        };

        Ok(CostSummary {
            total_cost,
            currency: "USD".into(),
            start_date,
            end_date,
            by_service,
            by_provider,
            previous_period_cost: Some(prev_cost),
            change_pct,
        })
    }

    pub async fn get_trend(
        pool: &PgPool,
        org_id: Uuid,
        start_date: NaiveDate,
        end_date: NaiveDate,
        _granularity: &str,
    ) -> Result<CostTrend, sqlx::Error> {
        let rows: Vec<(NaiveDate, f64)> = sqlx::query_as(
            "SELECT date, COALESCE(SUM(amount::float8), 0) as total FROM costs WHERE organization_id = $1 AND date >= $2 AND date <= $3 GROUP BY date ORDER BY date"
        )
        .bind(org_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(pool)
        .await?;

        let data_points: Vec<CostDataPoint> = rows
            .iter()
            .map(|(date, amount)| CostDataPoint {
                date: *date,
                amount: *amount,
                provider: None,
                service: None,
            })
            .collect();

        let total_cost: f64 = rows.iter().map(|(_, a)| a).sum();
        let days = rows.len().max(1) as f64;

        Ok(CostTrend {
            start_date,
            end_date,
            granularity: "daily".into(),
            data_points,
            total_cost,
            avg_daily_cost: total_cost / days,
        })
    }

    pub async fn get_breakdown(
        pool: &PgPool,
        org_id: Uuid,
        start_date: NaiveDate,
        end_date: NaiveDate,
        dimension: &str,
    ) -> Result<CostBreakdown, sqlx::Error> {
        let col = match dimension {
            "provider" => "provider",
            "account_id" => "account_id",
            "region" => "region",
            _ => "service",
        };

        let query = format!(
            "SELECT {col}, COALESCE(SUM(amount::float8), 0) as total FROM costs WHERE organization_id = $1 AND date >= $2 AND date <= $3 GROUP BY {col} ORDER BY total DESC"
        );

        let rows: Vec<(String, f64)> = sqlx::query_as(&query)
            .bind(org_id)
            .bind(start_date)
            .bind(end_date)
            .fetch_all(pool)
            .await?;

        let grand_total: f64 = rows.iter().map(|(_, a)| a).sum();
        let items: Vec<CostBreakdownItem> = rows
            .into_iter()
            .map(|(name, amount)| CostBreakdownItem {
                name,
                amount,
                percentage: if grand_total > 0.0 { (amount / grand_total) * 100.0 } else { 0.0 },
            })
            .collect();

        Ok(CostBreakdown {
            dimension: dimension.into(),
            items,
            total: grand_total,
            currency: "USD".into(),
        })
    }

    pub async fn get_daily_totals(
        pool: &PgPool,
        org_id: Uuid,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<(NaiveDate, f64)>, sqlx::Error> {
        sqlx::query_as(
            "SELECT date, COALESCE(SUM(amount::float8), 0) FROM costs WHERE organization_id = $1 AND date >= $2 AND date <= $3 GROUP BY date ORDER BY date"
        )
        .bind(org_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(pool)
        .await
    }

    pub async fn get_period_total(
        pool: &PgPool,
        org_id: Uuid,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<f64, sqlx::Error> {
        let row: (Option<f64>,) = sqlx::query_as(
            "SELECT COALESCE(SUM(amount::float8), 0) FROM costs WHERE organization_id = $1 AND date >= $2 AND date <= $3"
        )
        .bind(org_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_one(pool)
        .await?;
        Ok(row.0.unwrap_or(0.0))
    }

    pub async fn export_csv(
        pool: &PgPool,
        org_id: Uuid,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<CostRecord>, sqlx::Error> {
        sqlx::query_as::<_, CostRecord>(
            "SELECT * FROM costs WHERE organization_id = $1 AND date >= $2 AND date <= $3 ORDER BY date DESC"
        )
        .bind(org_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(pool)
        .await
    }

    /// Region-level spend totals for carbon estimation.
    pub async fn get_spend_by_region(
        pool: &PgPool,
        org_id: Uuid,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<(String, f64)>, sqlx::Error> {
        sqlx::query_as(
            "SELECT region, COALESCE(SUM(amount::float8), 0) as total \
             FROM costs \
             WHERE organization_id = $1 AND date >= $2 AND date <= $3 \
             GROUP BY region ORDER BY total DESC"
        )
        .bind(org_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(pool)
        .await
    }

    /// Monthly spend per region for carbon trend.
    pub async fn get_monthly_spend_by_region(
        pool: &PgPool,
        org_id: Uuid,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<(String, String, f64)>, sqlx::Error> {
        sqlx::query_as(
            "SELECT to_char(date, 'YYYY-MM') as month, region, COALESCE(SUM(amount::float8), 0) as total \
             FROM costs \
             WHERE organization_id = $1 AND date >= $2 AND date <= $3 \
             GROUP BY month, region ORDER BY month, total DESC"
        )
        .bind(org_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(pool)
        .await
    }

    /// AI/ML and GPU cost summary: filters for AI services and GPU instance types.
    pub async fn get_ai_summary(
        pool: &PgPool,
        org_id: Uuid,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<AiCostSummary, sqlx::Error> {
        let ai_filter = r#"
            organization_id = $1 AND date >= $2 AND date <= $3
            AND (
                service ILIKE '%SageMaker%'
                OR service ILIKE '%Bedrock%'
                OR service ILIKE '%Rekognition%'
                OR service ILIKE '%Comprehend%'
                OR service ILIKE '%Textract%'
                OR service ILIKE '%Polly%'
                OR service ILIKE '%Transcribe%'
                OR resource_id ~* '(p3|p4d?|g4dn|g5|inf1|trn1)\.'
            )
        "#;

        // Total AI spend
        let total_query = format!(
            "SELECT COALESCE(SUM(amount::float8), 0) FROM costs WHERE {ai_filter}"
        );
        let ai_total: (Option<f64>,) = sqlx::query_as(&total_query)
            .bind(org_id)
            .bind(start_date)
            .bind(end_date)
            .fetch_one(pool)
            .await?;
        let total_ai_spend = ai_total.0.unwrap_or(0.0);

        // Overall spend for share calculation
        let overall_total: (Option<f64>,) = sqlx::query_as(
            "SELECT COALESCE(SUM(amount::float8), 0) FROM costs WHERE organization_id = $1 AND date >= $2 AND date <= $3"
        )
        .bind(org_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_one(pool)
        .await?;
        let overall = overall_total.0.unwrap_or(0.0);

        // Breakdown by service
        let svc_query = format!(
            "SELECT service, COALESCE(SUM(amount::float8), 0) as total \
             FROM costs WHERE {ai_filter} \
             GROUP BY service ORDER BY total DESC"
        );
        let svc_rows: Vec<(String, f64)> = sqlx::query_as(&svc_query)
            .bind(org_id)
            .bind(start_date)
            .bind(end_date)
            .fetch_all(pool)
            .await?;

        let by_service: Vec<CostBreakdownItem> = svc_rows
            .into_iter()
            .map(|(name, amount)| CostBreakdownItem {
                name,
                percentage: if total_ai_spend > 0.0 {
                    (amount / total_ai_spend) * 100.0
                } else {
                    0.0
                },
                amount,
            })
            .collect();

        // Daily trend
        let trend_query = format!(
            "SELECT date, COALESCE(SUM(amount::float8), 0) as total \
             FROM costs WHERE {ai_filter} \
             GROUP BY date ORDER BY date"
        );
        let trend_rows: Vec<(NaiveDate, f64)> = sqlx::query_as(&trend_query)
            .bind(org_id)
            .bind(start_date)
            .bind(end_date)
            .fetch_all(pool)
            .await?;

        let trend: Vec<CostDataPoint> = trend_rows
            .into_iter()
            .map(|(date, amount)| CostDataPoint {
                date,
                amount,
                provider: None,
                service: None,
            })
            .collect();

        let ai_share_pct = if overall > 0.0 {
            (total_ai_spend / overall) * 100.0
        } else {
            0.0
        };

        Ok(AiCostSummary {
            total_ai_spend,
            currency: "USD".into(),
            start_date,
            end_date,
            by_service,
            trend,
            ai_share_pct,
        })
    }
}
