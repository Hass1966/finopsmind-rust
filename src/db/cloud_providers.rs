use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::CloudProviderConfig;

pub struct CloudProviderRepo;

impl CloudProviderRepo {
    pub async fn create(pool: &PgPool, config: &CloudProviderConfig) -> Result<CloudProviderConfig, sqlx::Error> {
        sqlx::query_as::<_, CloudProviderConfig>(
            r#"INSERT INTO cloud_providers (id, organization_id, provider_type, name, credentials, enabled, status)
               VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *"#,
        )
        .bind(config.id)
        .bind(config.organization_id)
        .bind(&config.provider_type)
        .bind(&config.name)
        .bind(&config.credentials)
        .bind(config.enabled)
        .bind(&config.status)
        .fetch_one(pool)
        .await
    }

    pub async fn list(pool: &PgPool, org_id: Uuid) -> Result<Vec<CloudProviderConfig>, sqlx::Error> {
        sqlx::query_as::<_, CloudProviderConfig>(
            "SELECT * FROM cloud_providers WHERE organization_id = $1 ORDER BY created_at DESC"
        )
        .bind(org_id)
        .fetch_all(pool)
        .await
    }

    pub async fn get_by_id(pool: &PgPool, org_id: Uuid, id: Uuid) -> Result<CloudProviderConfig, sqlx::Error> {
        sqlx::query_as::<_, CloudProviderConfig>(
            "SELECT * FROM cloud_providers WHERE id = $1 AND organization_id = $2"
        )
        .bind(id)
        .bind(org_id)
        .fetch_one(pool)
        .await
    }

    pub async fn update(pool: &PgPool, org_id: Uuid, id: Uuid, name: Option<&str>, credentials: Option<&[u8]>, enabled: Option<bool>) -> Result<CloudProviderConfig, sqlx::Error> {
        let existing = Self::get_by_id(pool, org_id, id).await?;
        let name = name.unwrap_or(&existing.name);
        let enabled = enabled.unwrap_or(existing.enabled);

        if let Some(creds) = credentials {
            sqlx::query_as::<_, CloudProviderConfig>(
                "UPDATE cloud_providers SET name = $1, credentials = $2, enabled = $3, updated_at = NOW() WHERE id = $4 AND organization_id = $5 RETURNING *"
            )
            .bind(name)
            .bind(creds)
            .bind(enabled)
            .bind(id)
            .bind(org_id)
            .fetch_one(pool)
            .await
        } else {
            sqlx::query_as::<_, CloudProviderConfig>(
                "UPDATE cloud_providers SET name = $1, enabled = $2, updated_at = NOW() WHERE id = $3 AND organization_id = $4 RETURNING *"
            )
            .bind(name)
            .bind(enabled)
            .bind(id)
            .bind(org_id)
            .fetch_one(pool)
            .await
        }
    }

    pub async fn delete(pool: &PgPool, org_id: Uuid, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM cloud_providers WHERE id = $1 AND organization_id = $2")
            .bind(id)
            .bind(org_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn update_status(pool: &PgPool, id: Uuid, status: &str, message: Option<&str>) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE cloud_providers SET status = $1, status_message = $2, updated_at = NOW() WHERE id = $3"
        )
        .bind(status)
        .bind(message)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn update_sync_time(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE cloud_providers SET last_sync_at = $1, status = 'connected', updated_at = NOW() WHERE id = $2"
        )
        .bind(Utc::now())
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn get_all_enabled(pool: &PgPool) -> Result<Vec<CloudProviderConfig>, sqlx::Error> {
        sqlx::query_as::<_, CloudProviderConfig>(
            "SELECT * FROM cloud_providers WHERE enabled = TRUE"
        )
        .fetch_all(pool)
        .await
    }
}
