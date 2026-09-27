PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

-- hash: xxh3 of the service and the attributes, so a resource is stored once.
CREATE TABLE IF NOT EXISTS resources (
  id INTEGER PRIMARY KEY,
  hash INTEGER NOT NULL UNIQUE,
  service TEXT NOT NULL,
  attributes TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS logs (
  ts INTEGER NOT NULL,
  resource_id INTEGER NOT NULL REFERENCES resources (id),
  severity INTEGER NOT NULL,
  body TEXT NOT NULL,
  trace_id BLOB,
  span_id BLOB,
  attributes TEXT NOT NULL,
  source TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS logs_ts ON logs (ts);
CREATE INDEX IF NOT EXISTS logs_trace_id ON logs (trace_id) WHERE trace_id IS NOT NULL;

-- Logs are never updated or deleted one by one: retention deletes the whole file.
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
  start_ts INTEGER NOT NULL,
  duration_ns INTEGER NOT NULL,
  status INTEGER NOT NULL,
  attributes TEXT NOT NULL,
  events TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS spans_trace_id ON spans (trace_id);
CREATE INDEX IF NOT EXISTS spans_start_ts ON spans (start_ts);

-- hash: xxh3 of the resource, the name, the kind, the unit, and the labels.
CREATE TABLE IF NOT EXISTS series (
  id INTEGER PRIMARY KEY,
  hash INTEGER NOT NULL UNIQUE,
  resource_id INTEGER NOT NULL REFERENCES resources (id),
  name TEXT NOT NULL,
  kind TEXT NOT NULL,
  unit TEXT NOT NULL,
  labels TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS series_name ON series (name);

CREATE TABLE IF NOT EXISTS points (
  series_id INTEGER NOT NULL REFERENCES series (id),
  ts INTEGER NOT NULL,
  value REAL NOT NULL,
  histogram TEXT
);
CREATE INDEX IF NOT EXISTS points_series_ts ON points (series_id, ts);
