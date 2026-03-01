use redis::AsyncCommands;
use serde::{de::DeserializeOwned, Serialize};

pub type RedisPool = redis::aio::ConnectionManager;

/// Get a cached value, or compute it and cache it.
pub async fn get_or_set<T, F, Fut>(
    redis: &RedisPool,
    key: &str,
    ttl_secs: u64,
    compute: F,
) -> anyhow::Result<T>
where
    T: Serialize + DeserializeOwned,
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    // Try to get from cache
    let mut conn = redis.clone();
    let cached: Option<String> = conn.get(key).await.ok().flatten();

    if let Some(json) = cached {
        if let Ok(value) = serde_json::from_str(&json) {
            tracing::debug!(key, "Cache hit");
            return Ok(value);
        }
    }

    // Compute and cache
    let value = compute().await?;
    let json = serde_json::to_string(&value)?;
    let _: Result<(), _> = conn.set_ex(key, &json, ttl_secs).await;
    tracing::debug!(key, ttl_secs, "Cache set");
    Ok(value)
}

/// Invalidate cache keys matching a pattern.
pub async fn invalidate_pattern(redis: &RedisPool, pattern: &str) -> anyhow::Result<()> {
    let mut conn = redis.clone();
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg(pattern)
        .query_async(&mut conn)
        .await
        .unwrap_or_default();

    if !keys.is_empty() {
        let _: Result<(), _> = redis::cmd("DEL")
            .arg(&keys)
            .query_async(&mut conn)
            .await;
        tracing::debug!(count = keys.len(), pattern, "Cache invalidated");
    }
    Ok(())
}
