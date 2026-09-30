PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS resources (
  id INTEGER PRIMARY KEY,
  -- xxh3 of the service and the attributes.
  hash INTEGER NOT NULL UNIQUE,
  service TEXT NOT NULL,
  -- A JSON object.
  attributes TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS logs (
  -- Unix nanoseconds.
  logged_at INTEGER NOT NULL,
  resource_id INTEGER NOT NULL REFERENCES resources (id),
  -- The OpenTelemetry severity number: 0 is unspecified, 1 to 4 trace, 5 to 8 debug,
  -- 9 to 12 info, 13 to 16 warn, 17 to 20 error, 21 to 24 fatal.
  severity INTEGER NOT NULL,
  body TEXT NOT NULL,
  -- 16 bytes. NULL for a log outside a span.
  trace_id BLOB,
  -- 8 bytes. NULL for a log outside a span.
  span_id BLOB,
  -- A JSON object.
  attributes TEXT NOT NULL,
  -- What took the log in. Only otlp so far.
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
  -- 16 bytes.
  trace_id BLOB NOT NULL,
  -- 8 bytes.
  span_id BLOB NOT NULL,
  -- 8 bytes. NULL for a root span.
  parent_span_id BLOB,
  resource_id INTEGER NOT NULL REFERENCES resources (id),
  name TEXT NOT NULL,
  -- The OpenTelemetry span kind: 0 unspecified, 1 internal, 2 server, 3 client,
  -- 4 producer, 5 consumer.
  kind INTEGER NOT NULL,
  -- Unix nanoseconds.
  started_at INTEGER NOT NULL,
  duration_ns INTEGER NOT NULL,
  -- The OpenTelemetry status code: 0 unset, 1 ok, 2 error.
  status INTEGER NOT NULL,
  -- A JSON object.
  attributes TEXT NOT NULL,
  -- A JSON array of objects with occurred_at in Unix nanoseconds, name, and attributes.
  events TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS spans_trace_id ON spans (trace_id);
CREATE INDEX IF NOT EXISTS spans_started_at ON spans (started_at);

CREATE TABLE IF NOT EXISTS series (
  id INTEGER PRIMARY KEY,
  -- xxh3 of the resource, the name, the kind, the temporality, the unit, and the labels.
  hash INTEGER NOT NULL UNIQUE,
  resource_id INTEGER NOT NULL REFERENCES resources (id),
  name TEXT NOT NULL,
  -- gauge, updown, counter, or histogram.
  kind TEXT NOT NULL,
  -- cumulative or delta for a counter and a histogram. NULL for the rest.
  temporality TEXT,
  unit TEXT NOT NULL,
  -- A JSON object.
  labels TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS series_name ON series (name);

CREATE TABLE IF NOT EXISTS points (
  series_id INTEGER NOT NULL REFERENCES series (id),
  -- Unix nanoseconds.
  recorded_at INTEGER NOT NULL,
  -- The sum of a histogram point.
  value REAL NOT NULL,
  -- The buckets of a histogram point as a JSON object: count, sum, min, max, and either
  -- bounds and counts, or scale, zero_count, positive, and negative for an exponential
  -- histogram. NULL for the other kinds.
  histogram TEXT,
  PRIMARY KEY (series_id, recorded_at)
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS attribute_keys (
  -- logs, spans, metrics for the labels of the series, resource, or span_names.
  key_group TEXT NOT NULL,
  key TEXT NOT NULL,
  -- The JSON type of the values: null, bool, int, float, string, array, or object, or
  -- mixed for more than one.
  value_type TEXT NOT NULL,
  -- How many records have the key.
  count INTEGER NOT NULL,
  -- 1 once the key has more distinct values than attribute_values keeps.
  has_more_values_than_listed INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (key_group, key)
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS attribute_values (
  -- As in attribute_keys. span_names holds the names of the spans under the key name.
  key_group TEXT NOT NULL,
  key TEXT NOT NULL,
  -- JSON.
  value TEXT NOT NULL,
  -- How many records have the value.
  count INTEGER NOT NULL,
  PRIMARY KEY (key_group, key, value)
) WITHOUT ROWID;
