use crate::api::Result;
use serde_json::Value;
use theseus::offline_queue::OfflineQueueEntry;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::<R>::new("queue")
        .invoke_handler(tauri::generate_handler![enqueue, list, remove,])
        .build()
}

#[tauri::command]
pub async fn enqueue(kind: String, state: Value) -> Result<String> {
    Ok(theseus::offline_queue::enqueue(&kind, &state).await?)
}

#[tauri::command]
pub async fn list() -> Result<Vec<OfflineQueueEntry>> {
    Ok(theseus::offline_queue::list().await?)
}

#[tauri::command]
pub async fn remove(id: String) -> Result<()> {
    Ok(theseus::offline_queue::remove(&id).await?)
}