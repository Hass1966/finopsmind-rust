use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{AutoApprovalRule, RemediationAction, RemediationSummary};

pub struct RemediationRepo;

impl RemediationRepo {
    pub async fn create_action(pool: &PgPool, action: &RemediationAction) -> Result<RemediationAction, sqlx::Error> {
        sqlx::query_as::<_, RemediationAction>(
            r#"INSERT INTO remediation_actions (id, organization_id, recommendation_id, type, status, provider, account_id, region, resource_id, resource_type,
               description, current_state, desired_state, estimated_savings, currency, risk, auto_approved, approval_rule, requested_by, audit_log)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
               RETURNING *"#,
        )
        .bind(action.id)
        .bind(action.organization_id)
        .bind(action.recommendation_id)
        .bind(&action.action_type)
        .bind(&action.status)
        .bind(&action.provider)
        .bind(&action.account_id)
        .bind(&action.region)
        .bind(&action.resource_id)
        .bind(&action.resource_type)
        .bind(&action.description)
        .bind(&action.current_state)
        .bind(&action.desired_state)
        .bind(action.estimated_savings)
        .bind(&action.currency)
        .bind(&action.risk)
        .bind(action.auto_approved)
        .bind(&action.approval_rule)
        .bind(action.requested_by)
        .bind(&action.audit_log)
        .fetch_one(pool)
        .await
    }

    pub async fn list_actions(pool: &PgPool, org_id: Uuid, limit: i64, offset: i64) -> Result<(Vec<RemediationAction>, i64), sqlx::Error> {
        let (total,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM remediation_actions WHERE organization_id = $1"
        )
        .bind(org_id)
        .fetch_one(pool)
        .await?;

        let actions = sqlx::query_as::<_, RemediationAction>(
            "SELECT * FROM remediation_actions WHERE organization_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
        )
        .bind(org_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok((actions, total))
    }

    pub async fn get_action(pool: &PgPool, org_id: Uuid, id: Uuid) -> Result<RemediationAction, sqlx::Error> {
        sqlx::query_as::<_, RemediationAction>(
            "SELECT * FROM remediation_actions WHERE id = $1 AND organization_id = $2"
        )
        .bind(id)
        .bind(org_id)
        .fetch_one(pool)
        .await
    }

    pub async fn update_action_status(
        pool: &PgPool,
        id: Uuid,
        status: &str,
        audit_log: &serde_json::Value,
    ) -> Result<RemediationAction, sqlx::Error> {
        sqlx::query_as::<_, RemediationAction>(
            "UPDATE remediation_actions SET status = $1, audit_log = $2, updated_at = NOW() WHERE id = $3 RETURNING *"
        )
        .bind(status)
        .bind(audit_log)
        .bind(id)
        .fetch_one(pool)
        .await
    }

    pub async fn approve_action(pool: &PgPool, id: Uuid, approved_by: Uuid, audit_log: &serde_json::Value) -> Result<RemediationAction, sqlx::Error> {
        sqlx::query_as::<_, RemediationAction>(
            "UPDATE remediation_actions SET status = 'approved', approved_by = $1, approved_at = NOW(), audit_log = $2, updated_at = NOW() WHERE id = $3 RETURNING *"
        )
        .bind(approved_by)
        .bind(audit_log)
        .bind(id)
        .fetch_one(pool)
        .await
    }

    pub async fn reject_action(pool: &PgPool, id: Uuid, approved_by: Uuid, reason: &str, audit_log: &serde_json::Value) -> Result<RemediationAction, sqlx::Error> {
        sqlx::query_as::<_, RemediationAction>(
            "UPDATE remediation_actions SET status = 'rejected', approved_by = $1, failure_reason = $2, audit_log = $3, updated_at = NOW() WHERE id = $4 RETURNING *"
        )
        .bind(approved_by)
        .bind(reason)
        .bind(audit_log)
        .bind(id)
        .fetch_one(pool)
        .await
    }

    pub async fn rollback_action(pool: &PgPool, id: Uuid, audit_log: &serde_json::Value) -> Result<RemediationAction, sqlx::Error> {
        sqlx::query_as::<_, RemediationAction>(
            "UPDATE remediation_actions SET status = 'rolled_back', rolled_back_at = NOW(), audit_log = $1, updated_at = NOW() WHERE id = $2 RETURNING *"
        )
        .bind(audit_log)
        .bind(id)
        .fetch_one(pool)
        .await
    }

    pub async fn get_summary(pool: &PgPool, org_id: Uuid) -> Result<RemediationSummary, sqlx::Error> {
        let row: (i64, i64, i64, i64, i64, i64, i64, f64) = sqlx::query_as(
            r#"SELECT
                COUNT(*),
                COUNT(*) FILTER (WHERE status = 'pending_approval'),
                COUNT(*) FILTER (WHERE status = 'approved'),
                COUNT(*) FILTER (WHERE status = 'executing'),
                COUNT(*) FILTER (WHERE status = 'completed'),
                COUNT(*) FILTER (WHERE status = 'failed'),
                COUNT(*) FILTER (WHERE status = 'rolled_back'),
                COALESCE(SUM(estimated_savings::float8) FILTER (WHERE status = 'completed'), 0)
               FROM remediation_actions WHERE organization_id = $1"#,
        )
        .bind(org_id)
        .fetch_one(pool)
        .await?;

        Ok(RemediationSummary {
            total: row.0,
            pending_approval: row.1,
            approved: row.2,
            executing: row.3,
            completed: row.4,
            failed: row.5,
            rolled_back: row.6,
            total_savings: row.7,
            currency: "USD".into(),
        })
    }

    // Auto-approval rules
    pub async fn list_rules(pool: &PgPool, org_id: Uuid) -> Result<Vec<AutoApprovalRule>, sqlx::Error> {
        sqlx::query_as::<_, AutoApprovalRule>(
            "SELECT * FROM auto_approval_rules WHERE organization_id = $1 ORDER BY created_at DESC"
        )
        .bind(org_id)
        .fetch_all(pool)
        .await
    }

    pub async fn get_active_rules(pool: &PgPool, org_id: Uuid) -> Result<Vec<AutoApprovalRule>, sqlx::Error> {
        sqlx::query_as::<_, AutoApprovalRule>(
            "SELECT * FROM auto_approval_rules WHERE organization_id = $1 AND enabled = TRUE"
        )
        .bind(org_id)
        .fetch_all(pool)
        .await
    }

    pub async fn create_rule(pool: &PgPool, org_id: Uuid, name: &str, enabled: bool, conditions: &serde_json::Value, created_by: Option<Uuid>) -> Result<AutoApprovalRule, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query_as::<_, AutoApprovalRule>(
            "INSERT INTO auto_approval_rules (id, organization_id, name, enabled, conditions, created_by) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *"
        )
        .bind(id)
        .bind(org_id)
        .bind(name)
        .bind(enabled)
        .bind(conditions)
        .bind(created_by)
        .fetch_one(pool)
        .await
    }

    pub async fn update_rule(pool: &PgPool, org_id: Uuid, id: Uuid, name: Option<&str>, enabled: Option<bool>, conditions: Option<&serde_json::Value>) -> Result<AutoApprovalRule, sqlx::Error> {
        let existing = sqlx::query_as::<_, AutoApprovalRule>(
            "SELECT * FROM auto_approval_rules WHERE id = $1 AND organization_id = $2"
        )
        .bind(id)
        .bind(org_id)
        .fetch_one(pool)
        .await?;

        let name = name.unwrap_or(&existing.name);
        let enabled = enabled.unwrap_or(existing.enabled);
        let conditions = conditions.unwrap_or(&existing.conditions);

        sqlx::query_as::<_, AutoApprovalRule>(
            "UPDATE auto_approval_rules SET name = $1, enabled = $2, conditions = $3, updated_at = NOW() WHERE id = $4 AND organization_id = $5 RETURNING *"
        )
        .bind(name)
        .bind(enabled)
        .bind(conditions)
        .bind(id)
        .bind(org_id)
        .fetch_one(pool)
        .await
    }

    pub async fn delete_rule(pool: &PgPool, org_id: Uuid, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM auto_approval_rules WHERE id = $1 AND organization_id = $2")
            .bind(id)
            .bind(org_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
