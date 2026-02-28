use sqlx::PgPool;
use uuid::Uuid;

use crate::models::Forecast;

pub struct ForecastRepo;

impl ForecastRepo {
    pub async fn create(pool: &PgPool, forecast: &Forecast) -> Result<Forecast, sqlx::Error> {
        sqlx::query_as::<_, Forecast>(
            r#"INSERT INTO forecasts (id, organization_id, generated_at, model_version, granularity, predictions, total_forecasted, confidence_level, currency, service_filter, account_filter)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
               RETURNING *"#,
        )
        .bind(forecast.id)
        .bind(forecast.organization_id)
        .bind(forecast.generated_at)
        .bind(&forecast.model_version)
        .bind(&forecast.granularity)
        .bind(&forecast.predictions)
        .bind(forecast.total_forecasted)
        .bind(forecast.confidence_level)
        .bind(&forecast.currency)
        .bind(&forecast.service_filter)
        .bind(&forecast.account_filter)
        .fetch_one(pool)
        .await
    }

    pub async fn list(pool: &PgPool, org_id: Uuid, limit: i64, offset: i64) -> Result<(Vec<Forecast>, i64), sqlx::Error> {
        let (total,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM forecasts WHERE organization_id = $1"
        )
        .bind(org_id)
        .fetch_one(pool)
        .await?;

        let forecasts = sqlx::query_as::<_, Forecast>(
            "SELECT * FROM forecasts WHERE organization_id = $1 ORDER BY generated_at DESC LIMIT $2 OFFSET $3"
        )
        .bind(org_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok((forecasts, total))
    }

    pub async fn get_latest(pool: &PgPool, org_id: Uuid) -> Result<Option<Forecast>, sqlx::Error> {
        sqlx::query_as::<_, Forecast>(
            "SELECT * FROM forecasts WHERE organization_id = $1 ORDER BY generated_at DESC LIMIT 1"
        )
        .bind(org_id)
        .fetch_optional(pool)
        .await
    }

    pub async fn get_by_id(pool: &PgPool, org_id: Uuid, id: Uuid) -> Result<Forecast, sqlx::Error> {
        sqlx::query_as::<_, Forecast>(
            "SELECT * FROM forecasts WHERE id = $1 AND organization_id = $2"
        )
        .bind(id)
        .bind(org_id)
        .fetch_one(pool)
        .await
    }
}
