CREATE TABLE api_cache (
	namespace TEXT NOT NULL,
	cache_key TEXT NOT NULL,
	data JSONB NOT NULL,
	updated INTEGER NOT NULL,
	PRIMARY KEY (namespace, cache_key)
);

CREATE INDEX api_cache_namespace ON api_cache(namespace);