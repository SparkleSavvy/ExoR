use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

/// A queued offline action, ready to be replayed once connectivity returns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfflineQueueEntry {
	pub id: String,
	pub kind: String,
	pub state: Value,
	pub status: String,
	pub created: i64,
}

/// Add an action to the offline queue, returning its id.
pub async fn enqueue(
	exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
	kind: &str,
	state: &Value,
) -> crate::Result<String> {
	let id = Uuid::new_v4().to_string();
	let state = serde_json::to_string(state)?;
	sqlx::query!(
		"INSERT INTO offline_queue (id, kind, state, created) VALUES (?, ?, jsonb(?), unixepoch())",
		id,
		kind,
		state,
	)
	.execute(exec)
	.await?;
	Ok(id)
}

/// List all queued actions, oldest first.
pub async fn list(
	exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
) -> crate::Result<Vec<OfflineQueueEntry>> {
	let res = sqlx::query!(
		"SELECT id, kind, json(state) AS \"state!: serde_json::Value\", status, created
		 FROM offline_queue ORDER BY created ASC"
	)
	.fetch_all(exec)
	.await?;
	Ok(res
		.into_iter()
		.map(|row| OfflineQueueEntry {
			id: row.id,
			kind: row.kind,
			state: row.state,
			status: row.status,
			created: row.created,
		})
		.collect())
}

/// Claim pending actions that are due for replay and mark their attempt time.
pub async fn dequeue_pending(
	exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
) -> crate::Result<Vec<OfflineQueueEntry>> {
	let res = sqlx::query!(
		"UPDATE offline_queue
		 SET attempted_after = unixepoch()
		 WHERE status = 'pending' AND (attempted_after IS NULL OR attempted_after < unixepoch())
		 RETURNING id, kind, json(state) AS \"state!: serde_json::Value\", status, created"
	)
	.fetch_all(exec)
	.await?;

	Ok(res
		.into_iter()
		.map(|row| OfflineQueueEntry {
			id: row.id,
			kind: row.kind,
			state: row.state,
			status: row.status,
			created: row.created,
		})
		.collect())
}

/// Remove a single queued action.
pub async fn remove(
	exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
	id: &str,
) -> crate::Result<()> {
	sqlx::query!("DELETE FROM offline_queue WHERE id = ?", id)
		.execute(exec)
		.await?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use sqlx::Connection;

	#[tokio::test]
	async fn offline_queue_roundtrip() {
		let options = sqlx::sqlite::SqliteConnectOptions::new()
			.in_memory(true)
			.create_if_missing(true);
		let mut conn =
			sqlx::sqlite::SqliteConnection::connect_with(&options).await.unwrap();
		sqlx::query(
			"CREATE TABLE offline_queue (id TEXT NOT NULL PRIMARY KEY, kind TEXT NOT NULL, state JSONB NOT NULL, status TEXT NOT NULL DEFAULT 'pending', created INTEGER NOT NULL, attempted_after INTEGER NULL)",
		)
		.execute(&mut conn)
		.await
		.unwrap();

		let state = serde_json::json!({ "project_id": "sodium", "version_ids": ["v1"] });
		let id = enqueue(&mut conn, "install_project", &state).await.unwrap();
		assert_eq!(list(&mut conn).await.unwrap().len(), 1);

		let pending = dequeue_pending(&mut conn).await.unwrap();
		assert_eq!(pending.len(), 1);
		assert_eq!(pending[0].kind, "install_project");
		assert_eq!(pending[0].state, state);
		// After being attempted once, dequeue_pending must not re-claim immediately.
		assert!(dequeue_pending(&mut conn).await.unwrap().is_empty());

		remove(&mut conn, &id).await.unwrap();
		assert!(list(&mut conn).await.unwrap().is_empty());
	}
}