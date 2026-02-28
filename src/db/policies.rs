use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{Policy, PolicySummary, PolicyViolation};

pub struct PolicyRepo;

impl PolicyRepo {
    pub async fn create(pool: &PgPool, policy: &Policy) -> Result<Policy, sqlx::Error> {
        sqlx::query_as::<_, Policy>(
            r#"INSERT INTO policies (id, organization_id, name, description, type, enforcement_mode, enabled, conditions, providers, environments, created_by)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) RETURNING *"#,
        )
        .bind(policy.id)
        .bind(policy.organization_id)
        .bind(&policy.name)
        .bind(&policy.description)
        .bind(&policy.policy_type)
        .bind(&policy.enforcement_mode)
        .bind(policy.enabled)
        .bind(&policy.conditions)
        .bind(&policy.providers)
        .bind(&policy.environments)
        .bind(policy.created_by)
        .fetch_one(pool)
        .await
    }

    pub async fn list(pool: &PgPool, org_id: Uuid, limit: i64, offset: i64) -> Result<(Vec<Policy>, i64), sqlx::Error> {
        let (total,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM policies WHERE organization_id = $1"
        )
        .bind(org_id)
        .fetch_one(pool)
        .await?;

        let policies = sqlx::query_as::<_, Policy>(
            "SELECT * FROM policies WHERE organization_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
        )
        .bind(org_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok((policies, total))
    }

    pub async fn get_by_id(pool: &PgPool, org_id: Uuid, id: Uuid) -> Result<Policy, sqlx::Error> {
        sqlx::query_as::<_, Policy>(
            "SELECT * FROM policies WHERE id = $1 AND organization_id = $2"
        )
        .bind(id)
        .bind(org_id)
        .fetch_one(pool)
        .await
    }

    pub async fn list_violations(
        pool: &PgPool,
        org_id: Uuid,
        policy_id: Option<Uuid>,
        status: Option<&str>,
        severity: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<PolicyViolation>, i64), sqlx::Error> {
        let mut conditions = vec!["organization_id = $1".to_string()];
        let mut idx = 2;

        if policy_id.is_some() {
            conditions.push(format!("policy_id = ${idx}"));
            idx += 1;
        }
        if status.is_some() {
            conditions.push(format!("status = ${idx}"));
            idx += 1;
        }
        if severity.is_some() {
            conditions.push(format!("severity = ${idx}"));
        }

        let where_str = conditions.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM policy_violations WHERE {where_str}");
        let mut count_q = sqlx::query_as::<_, (i64,)>(&count_sql).bind(org_id);
        if let Some(p) = policy_id { count_q = count_q.bind(p); }
        if let Some(s) = status { count_q = count_q.bind(s); }
        if let Some(s) = severity { count_q = count_q.bind(s); }
        let (total,) = count_q.fetch_one(pool).await?;

        let data_sql = format!(
            "SELECT * FROM policy_violations WHERE {where_str} ORDER BY detected_at DESC LIMIT {limit} OFFSET {offset}"
        );
        let mut data_q = sqlx::query_as::<_, PolicyViolation>(&data_sql).bind(org_id);
        if let Some(p) = policy_id { data_q = data_q.bind(p); }
        if let Some(s) = status { data_q = data_q.bind(s); }
        if let Some(s) = severity { data_q = data_q.bind(s); }
        let violations = data_q.fetch_all(pool).await?;

        Ok((violations, total))
    }

    pub async fn get_summary(pool: &PgPool, org_id: Uuid) -> Result<PolicySummary, sqlx::Error> {
        let policy_row: (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), COUNT(*) FILTER (WHERE enabled = TRUE) FROM policies WHERE organization_id = $1"
        )
        .bind(org_id)
        .fetch_one(pool)
        .await?;

        let violation_row: (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), COUNT(*) FILTER (WHERE status = 'open') FROM policy_violations WHERE organization_id = $1"
        )
        .bind(org_id)
        .fetch_one(pool)
        .await?;

        let type_rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT type, COUNT(*) FROM policies WHERE organization_id = $1 GROUP BY type"
        )
        .bind(org_id)
        .fetch_all(pool)
        .await?;

        let by_type: serde_json::Value = type_rows.into_iter()
            .map(|(t, c)| (t, serde_json::Value::from(c)))
            .collect::<serde_json::Map<String, serde_json::Value>>()
            .into();

        let sev_rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT severity, COUNT(*) FROM policy_violations WHERE organization_id = $1 AND status = 'open' GROUP BY severity"
        )
        .bind(org_id)
        .fetch_all(pool)
        .await?;

        let by_severity: serde_json::Value = sev_rows.into_iter()
            .map(|(s, c)| (s, serde_json::Value::from(c)))
            .collect::<serde_json::Map<String, serde_json::Value>>()
            .into();

        Ok(PolicySummary {
            total_policies: policy_row.0,
            enabled_policies: policy_row.1,
            total_violations: violation_row.0,
            open_violations: violation_row.1,
            by_type,
            by_severity,
        })
    }
}
