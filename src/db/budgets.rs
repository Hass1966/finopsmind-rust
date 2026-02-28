use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{Budget, BudgetSummary, CreateBudgetRequest, UpdateBudgetRequest};

pub struct BudgetRepo;

impl BudgetRepo {
    pub async fn list(pool: &PgPool, org_id: Uuid) -> Result<Vec<Budget>, sqlx::Error> {
        sqlx::query_as::<_, Budget>(
            "SELECT * FROM budgets WHERE organization_id = $1 ORDER BY created_at DESC"
        )
        .bind(org_id)
        .fetch_all(pool)
        .await
    }

    pub async fn get_by_id(pool: &PgPool, org_id: Uuid, id: Uuid) -> Result<Budget, sqlx::Error> {
        sqlx::query_as::<_, Budget>(
            "SELECT * FROM budgets WHERE id = $1 AND organization_id = $2"
        )
        .bind(id)
        .bind(org_id)
        .fetch_one(pool)
        .await
    }

    pub async fn create(pool: &PgPool, org_id: Uuid, req: &CreateBudgetRequest) -> Result<Budget, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query_as::<_, Budget>(
            r#"INSERT INTO budgets (id, organization_id, name, amount, currency, period, filters, thresholds, start_date, end_date)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
               RETURNING *"#,
        )
        .bind(id)
        .bind(org_id)
        .bind(&req.name)
        .bind(rust_decimal::Decimal::from_f64_retain(req.amount).unwrap_or_default())
        .bind(&req.currency)
        .bind(&req.period)
        .bind(&req.filters)
        .bind(&req.thresholds)
        .bind(req.start_date)
        .bind(req.end_date)
        .fetch_one(pool)
        .await
    }

    pub async fn update(pool: &PgPool, org_id: Uuid, id: Uuid, req: &UpdateBudgetRequest) -> Result<Budget, sqlx::Error> {
        let existing = Self::get_by_id(pool, org_id, id).await?;

        let name = req.name.as_deref().unwrap_or(&existing.name);
        let amount = req.amount.map(|a| rust_decimal::Decimal::from_f64_retain(a).unwrap_or_default()).unwrap_or(existing.amount);
        let period = req.period.as_deref().unwrap_or(&existing.period);
        let filters = req.filters.as_ref().unwrap_or(&existing.filters);
        let thresholds = req.thresholds.as_ref().unwrap_or(&existing.thresholds);
        let status = req.status.as_deref().unwrap_or(&existing.status);

        sqlx::query_as::<_, Budget>(
            r#"UPDATE budgets SET name = $1, amount = $2, period = $3, filters = $4, thresholds = $5, status = $6, updated_at = NOW()
               WHERE id = $7 AND organization_id = $8 RETURNING *"#,
        )
        .bind(name)
        .bind(amount)
        .bind(period)
        .bind(filters)
        .bind(thresholds)
        .bind(status)
        .bind(id)
        .bind(org_id)
        .fetch_one(pool)
        .await
    }

    pub async fn delete(pool: &PgPool, org_id: Uuid, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM budgets WHERE id = $1 AND organization_id = $2")
            .bind(id)
            .bind(org_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn update_spend(pool: &PgPool, id: Uuid, current_spend: f64, status: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE budgets SET current_spend = $1, status = $2, updated_at = NOW() WHERE id = $3"
        )
        .bind(rust_decimal::Decimal::from_f64_retain(current_spend).unwrap_or_default())
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn get_all(pool: &PgPool) -> Result<Vec<Budget>, sqlx::Error> {
        sqlx::query_as::<_, Budget>("SELECT * FROM budgets WHERE status != 'inactive'")
            .fetch_all(pool)
            .await
    }

    pub async fn get_summary(pool: &PgPool, org_id: Uuid) -> Result<BudgetSummary, sqlx::Error> {
        let row: (i64, i64, i64, i64, f64, f64) = sqlx::query_as(
            r#"SELECT
                COUNT(*),
                COUNT(*) FILTER (WHERE status = 'active'),
                COUNT(*) FILTER (WHERE status = 'warning'),
                COUNT(*) FILTER (WHERE status = 'exceeded'),
                COALESCE(SUM(amount::float8), 0),
                COALESCE(SUM(current_spend::float8), 0)
               FROM budgets WHERE organization_id = $1"#,
        )
        .bind(org_id)
        .fetch_one(pool)
        .await?;

        Ok(BudgetSummary {
            total_budgets: row.0,
            active_count: row.1,
            warning_count: row.2,
            exceeded_count: row.3,
            total_allocated: row.4,
            total_spent: row.5,
            currency: "USD".into(),
        })
    }
}
