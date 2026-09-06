use crate::api::Result;
use serde_json::Value;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::<R>::new("api-cache")
        .invoke_handler(tauri::generate_handler![
            get_cached,
            set_cached,
            delete_cached,
            clear_cached,
            keys_cached,
        ])
        .build()
}

#[tauri::command]
pub async fn get_cached(namespace: &str, key: &str) -> Result<Option<Value>> {
    Ok(theseus::api_cache::get(namespace, key).await?)
}

#[tauri::command]
pub async fn set_cached(namespace: &str, key: &str, data: Value) -> Result<()> {
    Ok(theseus::api_cache::set(namespace, key, &data).await?)
}

#[tauri::command]
pub async fn delete_cached(namespace: &str, key: &str) -> Result<()> {
    Ok(theseus::api_cache::delete_entry(namespace, key).await?)
}

#[tauri::command]
pub async fn clear_cached(namespace: Option<String>) -> Result<()> {
    Ok(theseus::api_cache::clear(namespace.as_deref()).await?)
}

#[tauri::command]
pub async fn keys_cached(namespace: &str) -> Result<Vec<String>> {
    Ok(theseus::api_cache::keys_in_namespace(namespace).await?)
}