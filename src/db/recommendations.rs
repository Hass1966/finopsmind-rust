use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{Recommendation, RecommendationSummary};

pub struct RecommendationRepo;

impl RecommendationRepo {
    pub async fn list(
        pool: &PgPool,
        org_id: Uuid,
        status: Option<&str>,
        rec_type: Option<&str>,
        impact: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<Recommendation>, i64), sqlx::Error> {
        let mut conditions = vec!["organization_id = $1".to_string()];
        let mut idx = 2;

        if status.is_some() {
            conditions.push(format!("status = ${idx}"));
            idx += 1;
        }
        if rec_type.is_some() {
            conditions.push(format!("type = ${idx}"));
            idx += 1;
        }
        if impact.is_some() {
            conditions.push(format!("impact = ${idx}"));
        }

        let where_str = conditions.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM recommendations WHERE {where_str}");
        let mut count_q = sqlx::query_as::<_, (i64,)>(&count_sql).bind(org_id);
        if let Some(s) = status { count_q = count_q.bind(s); }
        if let Some(t) = rec_type { count_q = count_q.bind(t); }
        if let Some(i) = impact { count_q = count_q.bind(i); }
        let (total,) = count_q.fetch_one(pool).await?;

        let data_sql = format!(
            "SELECT * FROM recommendations WHERE {where_str} ORDER BY estimated_savings DESC LIMIT {limit} OFFSET {offset}"
        );
        let mut data_q = sqlx::query_as::<_, Recommendation>(&data_sql).bind(org_id);
        if let Some(s) = status { data_q = data_q.bind(s); }
        if let Some(t) = rec_type { data_q = data_q.bind(t); }
        if let Some(i) = impact { data_q = data_q.bind(i); }
        let recs = data_q.fetch_all(pool).await?;

        Ok((recs, total))
    }

    pub async fn get_by_id(pool: &PgPool, org_id: Uuid, id: Uuid) -> Result<Recommendation, sqlx::Error> {
        sqlx::query_as::<_, Recommendation>(
            "SELECT * FROM recommendations WHERE id = $1 AND organization_id = $2"
        )
        .bind(id)
        .bind(org_id)
        .fetch_one(pool)
        .await
    }

    pub async fn update_status(pool: &PgPool, org_id: Uuid, id: Uuid, status: &str, notes: Option<&str>) -> Result<Recommendation, sqlx::Error> {
        sqlx::query_as::<_, Recommendation>(
            "UPDATE recommendations SET status = $1, notes = COALESCE($2, notes), updated_at = NOW() WHERE id = $3 AND organization_id = $4 RETURNING *"
        )
        .bind(status)
        .bind(notes)
        .bind(id)
        .bind(org_id)
        .fetch_one(pool)
        .await
    }

    pub async fn create(pool: &PgPool, rec: &Recommendation) -> Result<Recommendation, sqlx::Error> {
        sqlx::query_as::<_, Recommendation>(
            r#"INSERT INTO recommendations (id, organization_id, type, provider, account_id, region, resource_id, resource_type,
               current_config, recommended_config, estimated_savings, estimated_savings_pct, currency, impact, effort, risk, status, details, rule_id, severity)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
               ON CONFLICT (id) DO UPDATE SET estimated_savings = EXCLUDED.estimated_savings, updated_at = NOW()
               RETURNING *"#,
        )
        .bind(rec.id)
        .bind(rec.organization_id)
        .bind(&rec.rec_type)
        .bind(&rec.provider)
        .bind(&rec.account_id)
        .bind(&rec.region)
        .bind(&rec.resource_id)
        .bind(&rec.resource_type)
        .bind(&rec.current_config)
        .bind(&rec.recommended_config)
        .bind(rec.estimated_savings)
        .bind(rec.estimated_savings_pct)
        .bind(&rec.currency)
        .bind(&rec.impact)
        .bind(&rec.effort)
        .bind(&rec.risk)
        .bind(&rec.status)
        .bind(&rec.details)
        .bind(&rec.rule_id)
        .bind(&rec.severity)
        .fetch_one(pool)
        .await
    }

    pub async fn get_summary(pool: &PgPool, org_id: Uuid) -> Result<RecommendationSummary, sqlx::Error> {
        let row: (i64, i64, i64, i64, f64, f64) = sqlx::query_as(
            r#"SELECT
                COUNT(*),
                COUNT(*) FILTER (WHERE status = 'pending'),
                COUNT(*) FILTER (WHERE status = 'implemented'),
                COUNT(*) FILTER (WHERE status = 'dismissed'),
                COALESCE(SUM(estimated_savings::float8) FILTER (WHERE status = 'pending'), 0),
                COALESCE(SUM(estimated_savings::float8) FILTER (WHERE status = 'implemented'), 0)
               FROM recommendations WHERE organization_id = $1"#,
        )
        .bind(org_id)
        .fetch_one(pool)
        .await?;

        // By type
        let type_rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT type, COUNT(*) FROM recommendations WHERE organization_id = $1 GROUP BY type"
        )
        .bind(org_id)
        .fetch_all(pool)
        .await?;

        let by_type: serde_json::Value = type_rows.into_iter()
            .map(|(t, c)| (t, serde_json::Value::from(c)))
            .collect::<serde_json::Map<String, serde_json::Value>>()
            .into();

        // By impact
        let impact_rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT impact, COUNT(*) FROM recommendations WHERE organization_id = $1 GROUP BY impact"
        )
        .bind(org_id)
        .fetch_all(pool)
        .await?;

        let by_impact: serde_json::Value = impact_rows.into_iter()
            .map(|(i, c)| (i, serde_json::Value::from(c)))
            .collect::<serde_json::Map<String, serde_json::Value>>()
            .into();

        Ok(RecommendationSummary {
            total_count: row.0,
            pending_count: row.1,
            implemented_count: row.2,
            dismissed_count: row.3,
            total_savings: row.4,
            implemented_savings: row.5,
            by_type,
            by_impact,
            currency: "USD".into(),
        })
    }
}
