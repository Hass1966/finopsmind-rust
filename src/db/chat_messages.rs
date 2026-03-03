use sqlx::PgPool;
use uuid::Uuid;

use crate::models::ChatMessage;

pub struct ChatMessageRepo;

impl ChatMessageRepo {
    /// Save a single chat message (user or assistant).
    pub async fn create(
        pool: &PgPool,
        org_id: Uuid,
        user_id: Uuid,
        role: &str,
        content: &str,
        intent: Option<&str>,
    ) -> Result<ChatMessage, sqlx::Error> {
        sqlx::query_as::<_, ChatMessage>(
            r#"INSERT INTO chat_messages (id, organization_id, user_id, role, content, intent)
               VALUES (uuid_generate_v4(), $1, $2, $3, $4, $5)
               RETURNING *"#,
        )
        .bind(org_id)
        .bind(user_id)
        .bind(role)
        .bind(content)
        .bind(intent)
        .fetch_one(pool)
        .await
    }

    /// Fetch the last N messages for a user, in chronological order (oldest first).
    /// Used for building LLM conversation context.
    pub async fn get_recent(
        pool: &PgPool,
        user_id: Uuid,
        limit: i64,
    ) -> Result<Vec<ChatMessage>, sqlx::Error> {
        sqlx::query_as::<_, ChatMessage>(
            r#"SELECT * FROM (
                 SELECT * FROM chat_messages
                 WHERE user_id = $1
                 ORDER BY created_at DESC
                 LIMIT $2
               ) sub ORDER BY created_at ASC"#,
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(pool)
        .await
    }

    /// Paginated history (newest first) for the history endpoint.
    pub async fn list(
        pool: &PgPool,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<ChatMessage>, i64), sqlx::Error> {
        let (total,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM chat_messages WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let messages = sqlx::query_as::<_, ChatMessage>(
            r#"SELECT * FROM chat_messages
               WHERE user_id = $1
               ORDER BY created_at DESC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok((messages, total))
    }

    /// Delete all messages for a user (clear conversation).
    pub async fn delete_all_for_user(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM chat_messages WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
