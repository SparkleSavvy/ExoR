CREATE TABLE offline_queue (
	id TEXT NOT NULL PRIMARY KEY,
	kind TEXT NOT NULL,
	state JSONB NOT NULL,
	status TEXT NOT NULL DEFAULT 'pending',
	created INTEGER NOT NULL,
	attempted_after INTEGER NULL
);

CREATE INDEX offline_queue_status ON offline_queue(status);