use crate::state::api_cache::{
    self, delete, keys, load, save,
};
use serde_json::Value;

/// Load a cached API response blob, if any.
pub async fn get(namespace: &str, key: &str) -> crate::Result<Option<Value>> {
    let state = crate::State::get().await?;
    load(&state.pool, namespace, key).await
}

/// Store a cached API response blob, upserting any existing entry.
pub async fn set(namespace: &str, key: &str, data: &Value) -> crate::Result<()> {
    let state = crate::State::get().await?;
    save(&state.pool, namespace, key, data).await
}

/// Delete a single cached API response blob.
pub async fn delete_entry(
    namespace: &str,
    key: &str,
) -> crate::Result<()> {
    let state = crate::State::get().await?;
    delete(&state.pool, namespace, key).await
}

/// Clear cached API response blobs, optionally scoped to a namespace.
pub async fn clear(namespace: Option<&str>) -> crate::Result<()> {
    let state = crate::State::get().await?;
    api_cache::clear(&state.pool, namespace).await
}

/// List cache keys within a namespace, most recently updated first.
pub async fn keys_in_namespace(namespace: &str) -> crate::Result<Vec<String>> {
    let state = crate::State::get().await?;
    keys(&state.pool, namespace).await
}