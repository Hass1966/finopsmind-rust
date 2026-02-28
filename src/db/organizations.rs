use sqlx::PgPool;
use uuid::Uuid;

use crate::models::Organization;

pub struct OrgRepo;

impl OrgRepo {
    pub async fn create(pool: &PgPool, name: &str, settings: &serde_json::Value) -> Result<Organization, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query_as::<_, Organization>(
            "INSERT INTO organizations (id, name, settings) VALUES ($1, $2, $3) RETURNING *"
        )
        .bind(id)
        .bind(name)
        .bind(settings)
        .fetch_one(pool)
        .await
    }

    pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Organization, sqlx::Error> {
        sqlx::query_as::<_, Organization>("SELECT * FROM organizations WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
    }

    pub async fn list(pool: &PgPool) -> Result<Vec<Organization>, sqlx::Error> {
        sqlx::query_as::<_, Organization>("SELECT * FROM organizations ORDER BY name")
            .fetch_all(pool)
            .await
    }

    pub async fn update_settings(pool: &PgPool, id: Uuid, settings: &serde_json::Value) -> Result<Organization, sqlx::Error> {
        sqlx::query_as::<_, Organization>(
            "UPDATE organizations SET settings = $1, updated_at = NOW() WHERE id = $2 RETURNING *"
        )
        .bind(settings)
        .bind(id)
        .fetch_one(pool)
        .await
    }
}
