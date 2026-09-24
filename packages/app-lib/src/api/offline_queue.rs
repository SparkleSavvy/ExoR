use crate::state::offline_queue;
use serde_json::Value;

pub use crate::state::offline_queue::OfflineQueueEntry;

/// Add an action to the offline queue.
pub async fn enqueue(kind: &str, state: &Value) -> crate::Result<String> {
    let app = crate::State::get().await?;
    offline_queue::enqueue(&app.pool, kind, state).await
}

/// List all queued actions, oldest first.
pub async fn list() -> crate::Result<Vec<OfflineQueueEntry>> {
    let app = crate::State::get().await?;
    offline_queue::list(&app.pool).await
}

/// Claim pending actions that are due for replay.
pub async fn dequeue_pending() -> crate::Result<Vec<OfflineQueueEntry>> {
    let app = crate::State::get().await?;
    offline_queue::dequeue_pending(&app.pool).await
}

/// Remove a single queued action.
pub async fn remove(id: &str) -> crate::Result<()> {
    let app = crate::State::get().await?;
    offline_queue::remove(&app.pool, id).await
}
