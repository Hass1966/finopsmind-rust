use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::User;

pub struct UserRepo;

impl UserRepo {
    pub async fn create(
        pool: &PgPool,
        org_id: Uuid,
        email: &str,
        password_hash: &str,
        first_name: &str,
        last_name: &str,
        role: &str,
    ) -> Result<User, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query_as::<_, User>(
            r#"INSERT INTO users (id, organization_id, email, password_hash, first_name, last_name, role)
               VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *"#,
        )
        .bind(id)
        .bind(org_id)
        .bind(email)
        .bind(password_hash)
        .bind(first_name)
        .bind(last_name)
        .bind(role)
        .fetch_one(pool)
        .await
    }

    pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<User, sqlx::Error> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
    }

    pub async fn get_by_email(pool: &PgPool, email: &str) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1 AND active = TRUE")
            .bind(email)
            .fetch_optional(pool)
            .await
    }

    pub async fn get_by_api_key_hash(pool: &PgPool, hash: &str) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE api_key_hash = $1 AND active = TRUE")
            .bind(hash)
            .fetch_optional(pool)
            .await
    }

    pub async fn update_last_login(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET last_login_at = $1, updated_at = NOW() WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn set_api_key_hash(pool: &PgPool, id: Uuid, hash: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET api_key_hash = $1, updated_at = NOW() WHERE id = $2")
            .bind(hash)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
