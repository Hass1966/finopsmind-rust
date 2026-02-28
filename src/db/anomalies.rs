use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{Anomaly, AnomalySummary, UpdateAnomalyRequest};

pub struct AnomalyRepo;

impl AnomalyRepo {
    pub async fn create(pool: &PgPool, anomaly: &Anomaly) -> Result<Anomaly, sqlx::Error> {
        sqlx::query_as::<_, Anomaly>(
            r#"INSERT INTO anomalies (id, organization_id, date, actual_amount, expected_amount, deviation, deviation_pct, score, severity, status, provider, service, account_id, region, root_cause, detected_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
               RETURNING *"#,
        )
        .bind(anomaly.id)
        .bind(anomaly.organization_id)
        .bind(anomaly.date)
        .bind(anomaly.actual_amount)
        .bind(anomaly.expected_amount)
        .bind(anomaly.deviation)
        .bind(anomaly.deviation_pct)
        .bind(anomaly.score)
        .bind(&anomaly.severity)
        .bind(&anomaly.status)
        .bind(&anomaly.provider)
        .bind(&anomaly.service)
        .bind(&anomaly.account_id)
        .bind(&anomaly.region)
        .bind(&anomaly.root_cause)
        .bind(anomaly.detected_at)
        .fetch_one(pool)
        .await
    }

    pub async fn create_batch(pool: &PgPool, anomalies: &[Anomaly]) -> Result<(), sqlx::Error> {
        let mut tx = pool.begin().await?;
        for a in anomalies {
            sqlx::query(
                r#"INSERT INTO anomalies (id, organization_id, date, actual_amount, expected_amount, deviation, deviation_pct, score, severity, status, provider, service, account_id, region, root_cause, detected_at)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)"#,
            )
            .bind(a.id)
            .bind(a.organization_id)
            .bind(a.date)
            .bind(a.actual_amount)
            .bind(a.expected_amount)
            .bind(a.deviation)
            .bind(a.deviation_pct)
            .bind(a.score)
            .bind(&a.severity)
            .bind(&a.status)
            .bind(&a.provider)
            .bind(&a.service)
            .bind(&a.account_id)
            .bind(&a.region)
            .bind(&a.root_cause)
            .bind(a.detected_at)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn list(
        pool: &PgPool,
        org_id: Uuid,
        severity: Option<&str>,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<Anomaly>, i64), sqlx::Error> {
        let mut where_clauses = vec!["organization_id = $1".to_string()];
        let mut param_idx = 2;

        if severity.is_some() {
            where_clauses.push(format!("severity = ${param_idx}"));
            param_idx += 1;
        }
        if status.is_some() {
            where_clauses.push(format!("status = ${param_idx}"));
        }

        let where_str = where_clauses.join(" AND ");

        // Build count query
        let count_sql = format!("SELECT COUNT(*) FROM anomalies WHERE {where_str}");
        let mut count_query = sqlx::query_as::<_, (i64,)>(&count_sql).bind(org_id);
        if let Some(s) = severity {
            count_query = count_query.bind(s);
        }
        if let Some(s) = status {
            count_query = count_query.bind(s);
        }
        let (total,) = count_query.fetch_one(pool).await?;

        // Build data query
        let data_sql = format!(
            "SELECT * FROM anomalies WHERE {where_str} ORDER BY detected_at DESC LIMIT {limit} OFFSET {offset}"
        );
        let mut data_query = sqlx::query_as::<_, Anomaly>(&data_sql).bind(org_id);
        if let Some(s) = severity {
            data_query = data_query.bind(s);
        }
        if let Some(s) = status {
            data_query = data_query.bind(s);
        }
        let anomalies = data_query.fetch_all(pool).await?;

        Ok((anomalies, total))
    }

    pub async fn get_by_id(pool: &PgPool, org_id: Uuid, id: Uuid) -> Result<Anomaly, sqlx::Error> {
        sqlx::query_as::<_, Anomaly>(
            "SELECT * FROM anomalies WHERE id = $1 AND organization_id = $2"
        )
        .bind(id)
        .bind(org_id)
        .fetch_one(pool)
        .await
    }

    pub async fn update(pool: &PgPool, org_id: Uuid, id: Uuid, req: &UpdateAnomalyRequest) -> Result<Anomaly, sqlx::Error> {
        let existing = Self::get_by_id(pool, org_id, id).await?;
        let status = req.status.as_deref().unwrap_or(&existing.status);
        let notes = req.notes.as_deref().or(existing.notes.as_deref());
        let root_cause = req.root_cause.as_deref().or(existing.root_cause.as_deref());

        sqlx::query_as::<_, Anomaly>(
            "UPDATE anomalies SET status = $1, notes = $2, root_cause = $3, updated_at = NOW() WHERE id = $4 AND organization_id = $5 RETURNING *"
        )
        .bind(status)
        .bind(notes)
        .bind(root_cause)
        .bind(id)
        .bind(org_id)
        .fetch_one(pool)
        .await
    }

    pub async fn acknowledge(pool: &PgPool, org_id: Uuid, id: Uuid, user_id: Uuid) -> Result<Anomaly, sqlx::Error> {
        sqlx::query_as::<_, Anomaly>(
            "UPDATE anomalies SET status = 'acknowledged', acknowledged_at = $1, acknowledged_by = $2, updated_at = NOW() WHERE id = $3 AND organization_id = $4 RETURNING *"
        )
        .bind(Utc::now())
        .bind(user_id)
        .bind(id)
        .bind(org_id)
        .fetch_one(pool)
        .await
    }

    pub async fn resolve(pool: &PgPool, org_id: Uuid, id: Uuid) -> Result<Anomaly, sqlx::Error> {
        sqlx::query_as::<_, Anomaly>(
            "UPDATE anomalies SET status = 'resolved', resolved_at = $1, updated_at = NOW() WHERE id = $2 AND organization_id = $3 RETURNING *"
        )
        .bind(Utc::now())
        .bind(id)
        .bind(org_id)
        .fetch_one(pool)
        .await
    }

    pub async fn get_summary(pool: &PgPool, org_id: Uuid) -> Result<AnomalySummary, sqlx::Error> {
        let row: (i64, i64, i64, i64, i64, i64, i64, i64, f64) = sqlx::query_as(
            r#"SELECT
                COUNT(*),
                COUNT(*) FILTER (WHERE status = 'open'),
                COUNT(*) FILTER (WHERE status = 'acknowledged'),
                COUNT(*) FILTER (WHERE status = 'resolved'),
                COUNT(*) FILTER (WHERE severity = 'critical'),
                COUNT(*) FILTER (WHERE severity = 'high'),
                COUNT(*) FILTER (WHERE severity = 'medium'),
                COUNT(*) FILTER (WHERE severity = 'low'),
                COALESCE(SUM((actual_amount - expected_amount)::float8) FILTER (WHERE status != 'resolved'), 0)
               FROM anomalies WHERE organization_id = $1"#,
        )
        .bind(org_id)
        .fetch_one(pool)
        .await?;

        Ok(AnomalySummary {
            total: row.0,
            open: row.1,
            acknowledged: row.2,
            resolved: row.3,
            critical: row.4,
            high: row.5,
            medium: row.6,
            low: row.7,
            total_impact: row.8,
            currency: "USD".into(),
        })
    }
}
