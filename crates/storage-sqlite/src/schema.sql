PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS resources (
  id INTEGER PRIMARY KEY,
  hash INTEGER NOT NULL UNIQUE,
  service TEXT NOT NULL,
  attributes TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS logs (
  logged_at INTEGER NOT NULL,
  resource_id INTEGER NOT NULL REFERENCES resources (id),
  severity INTEGER NOT NULL,
  body TEXT NOT NULL,
  trace_id BLOB,
  span_id BLOB,
  attributes TEXT NOT NULL,
  source TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS logs_logged_at ON logs (logged_at);
CREATE INDEX IF NOT EXISTS logs_trace_id ON logs (trace_id) WHERE trace_id IS NOT NULL;

-- Retention deletes whole files, so no trigger follows an update or a delete.
CREATE VIRTUAL TABLE IF NOT EXISTS logs_fts USING fts5 (body, content = 'logs');
CREATE TRIGGER IF NOT EXISTS logs_fts_insert AFTER INSERT ON logs BEGIN
  INSERT INTO logs_fts (rowid, body) VALUES (new.rowid, new.body);
END;

CREATE TABLE IF NOT EXISTS spans (
  trace_id BLOB NOT NULL,
  span_id BLOB NOT NULL,
  parent_span_id BLOB,
  resource_id INTEGER NOT NULL REFERENCES resources (id),
  name TEXT NOT NULL,
  kind INTEGER NOT NULL,
  started_at INTEGER NOT NULL,
  duration_ns INTEGER NOT NULL,
  status INTEGER NOT NULL,
  attributes TEXT NOT NULL,
  events TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS spans_trace_id ON spans (trace_id);
CREATE INDEX IF NOT EXISTS spans_started_at ON spans (started_at);

CREATE TABLE IF NOT EXISTS series (
  id INTEGER PRIMARY KEY,
  hash INTEGER NOT NULL UNIQUE,
  resource_id INTEGER NOT NULL REFERENCES resources (id),
  name TEXT NOT NULL,
  kind TEXT NOT NULL,
  temporality TEXT,
  unit TEXT NOT NULL,
  labels TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS series_name ON series (name);

CREATE TABLE IF NOT EXISTS points (
  series_id INTEGER NOT NULL REFERENCES series (id),
  recorded_at INTEGER NOT NULL,
  value REAL NOT NULL,
  histogram TEXT,
  PRIMARY KEY (series_id, recorded_at)
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS attribute_keys (
  key_group TEXT NOT NULL,
  key TEXT NOT NULL,
  value_type TEXT NOT NULL,
  count INTEGER NOT NULL,
  has_more_values INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (key_group, key)
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS attribute_values (
  key_group TEXT NOT NULL,
  key TEXT NOT NULL,
  value TEXT NOT NULL,
  count INTEGER NOT NULL,
  PRIMARY KEY (key_group, key, value)
) WITHOUT ROWID;
