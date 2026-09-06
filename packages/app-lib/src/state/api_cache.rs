use serde_json::Value;

/// Load a cached API response blob, if any.
pub async fn load(
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    namespace: &str,
    cache_key: &str,
) -> crate::Result<Option<Value>> {
    let res = sqlx::query!(
        "SELECT json(data) AS \"data!: serde_json::Value\" FROM api_cache WHERE namespace = ? AND cache_key = ?",
        namespace,
        cache_key
    )
    .fetch_optional(exec)
    .await?;
    Ok(res.map(|row| row.data))
}

/// Store a cached API response blob, upserting any existing entry.
pub async fn save(
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    namespace: &str,
    cache_key: &str,
    data: &Value,
) -> crate::Result<()> {
    let data = serde_json::to_string(data)?;
    sqlx::query!(
        "INSERT INTO api_cache (namespace, cache_key, data, updated) VALUES (?, ?, jsonb(?), unixepoch())
         ON CONFLICT (namespace, cache_key) DO UPDATE SET data = excluded.data, updated = excluded.updated",
        namespace,
        cache_key,
        data,
    )
    .execute(exec)
    .await?;
    Ok(())
}

/// Delete a single cached response blob.
pub async fn delete(
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    namespace: &str,
    cache_key: &str,
) -> crate::Result<()> {
    sqlx::query!(
        "DELETE FROM api_cache WHERE namespace = ? AND cache_key = ?",
        namespace,
        cache_key
    )
    .execute(exec)
    .await?;
    Ok(())
}

/// Clear cached response blobs, optionally scoped to a namespace.
pub async fn clear(
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    namespace: Option<&str>,
) -> crate::Result<()> {
    if let Some(namespace) = namespace {
        sqlx::query!("DELETE FROM api_cache WHERE namespace = ?", namespace)
            .execute(exec)
            .await?;
    } else {
        sqlx::query!("DELETE FROM api_cache").execute(exec).await?;
    }
    Ok(())
}

/// List cache keys within a namespace, most recently updated first.
pub async fn keys(
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    namespace: &str,
) -> crate::Result<Vec<String>> {
    let res = sqlx::query!(
        "SELECT cache_key FROM api_cache WHERE namespace = ? ORDER BY updated DESC",
        namespace
    )
    .fetch_all(exec)
    .await?;
    Ok(res.into_iter().map(|row| row.cache_key).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Connection;

    #[tokio::test]
    async fn api_cache_roundtrip() {
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .in_memory(true)
            .create_if_missing(true);
        let mut conn =
            sqlx::sqlite::SqliteConnection::connect_with(&options).await.unwrap();
        sqlx::query(
            "CREATE TABLE api_cache (namespace TEXT NOT NULL, cache_key TEXT NOT NULL, data JSONB NOT NULL, updated INTEGER NOT NULL, PRIMARY KEY (namespace, cache_key))",
        )
        .execute(&mut conn)
        .await
        .unwrap();

        assert!(load(&mut conn, "search", "q=test").await.unwrap().is_none());
        save(&mut conn, "search", "q=test", &serde_json::json!({ "hits": [] }))
            .await
            .unwrap();
        assert!(load(&mut conn, "search", "q=test").await.unwrap().is_some());
        assert_eq!(keys(&mut conn, "search").await.unwrap(), vec!["q=test"]);
        delete(&mut conn, "search", "q=test").await.unwrap();
        assert!(load(&mut conn, "search", "q=test").await.unwrap().is_none());
    }
}