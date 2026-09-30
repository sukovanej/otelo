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

-- hash: xxh3 of the resource, the name, the kind, the temporality, the unit,
-- and the labels. kind is gauge, updown, counter, or histogram. temporality is
-- cumulative or delta for a counter and a histogram, and NULL for the rest.
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

-- The key stores a point once and makes a batch that is sent again overwrite
-- its rows.
CREATE TABLE IF NOT EXISTS points (
  series_id INTEGER NOT NULL REFERENCES series (id),
  ts INTEGER NOT NULL,
  -- The sum of a histogram point.
  value REAL NOT NULL,
  -- The buckets of a histogram point as JSON: count, sum, min, max, and either
  -- bounds and counts, or scale, zero_count, positive, and negative for an
  -- exponential histogram. NULL for the other kinds.
  histogram TEXT,
  PRIMARY KEY (series_id, ts)
) WITHOUT ROWID;

-- The attribute keys the records carry, so a query can complete them. signal
-- is logs, spans, metrics (series labels), or resource. type is the JSON type
-- of the values, or mixed.
CREATE TABLE IF NOT EXISTS attribute_keys (
  signal TEXT NOT NULL,
  key TEXT NOT NULL,
  type TEXT NOT NULL,
  count INTEGER NOT NULL,
  -- 1 once the key has more distinct values than attribute_values keeps.
  many_values INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (signal, key)
) WITHOUT ROWID;

-- Up to 200 values of each key, as JSON, with how often they occur. The
-- signal span_names holds the names of the spans under the key name.
CREATE TABLE IF NOT EXISTS attribute_values (
  signal TEXT NOT NULL,
  key TEXT NOT NULL,
  value TEXT NOT NULL,
  count INTEGER NOT NULL,
  PRIMARY KEY (signal, key, value)
) WITHOUT ROWID;
