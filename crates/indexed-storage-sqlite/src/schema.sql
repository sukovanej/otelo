-- Retention deletes rows, and only a file made with it can give their pages back to the disk.
PRAGMA auto_vacuum = INCREMENTAL;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS resources (
  id INTEGER PRIMARY KEY,
  -- xxh3 of the service and the attributes.
  identity_hash INTEGER NOT NULL UNIQUE,
  service TEXT NOT NULL,
  -- A JSON object.
  attributes TEXT NOT NULL
);
-- Without it SQLite reads every span or log of the range to find those of one service.
CREATE INDEX IF NOT EXISTS resources_service ON resources (service);

CREATE TABLE IF NOT EXISTS logs (
  -- Unix nanoseconds.
  logged_at INTEGER NOT NULL,
  resource_id INTEGER NOT NULL REFERENCES resources (id),
  -- The OpenTelemetry severity number: 0 is unspecified, 1 to 4 trace, 5 to 8 debug,
  -- 9 to 12 info, 13 to 16 warn, 17 to 20 error, 21 to 24 fatal.
  severity_number INTEGER NOT NULL,
  body TEXT NOT NULL,
  -- 16 bytes. NULL for a log outside a span.
  trace_id BLOB,
  -- 8 bytes. NULL for a log outside a span.
  span_id BLOB,
  -- A JSON object.
  attributes TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS logs_logged_at ON logs (logged_at);
CREATE INDEX IF NOT EXISTS logs_trace_id ON logs (trace_id) WHERE trace_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS logs_resource_id_logged_at ON logs (resource_id, logged_at);

CREATE VIRTUAL TABLE IF NOT EXISTS log_body_search USING fts5 (body, content = 'logs');
CREATE TRIGGER IF NOT EXISTS log_body_search_insert AFTER INSERT ON logs BEGIN
  INSERT INTO log_body_search (rowid, body) VALUES (new.rowid, new.body);
END;
-- Retention deletes logs, and no log is ever updated.
CREATE TRIGGER IF NOT EXISTS log_body_search_delete AFTER DELETE ON logs BEGIN
  INSERT INTO log_body_search (log_body_search, rowid, body) VALUES ('delete', old.rowid, old.body);
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
  status_code INTEGER NOT NULL,
  -- A JSON object.
  attributes TEXT NOT NULL,
  -- A JSON array of objects with occurred_at in Unix nanoseconds, name, and attributes.
  events TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS spans_trace_id ON spans (trace_id);
CREATE INDEX IF NOT EXISTS spans_started_at ON spans (started_at);
CREATE INDEX IF NOT EXISTS spans_resource_id_started_at ON spans (resource_id, started_at);

CREATE TABLE IF NOT EXISTS metric_series (
  id INTEGER PRIMARY KEY,
  -- xxh3 of the resource, the name, the kind, the aggregation temporality, the unit, and the
  -- attributes.
  identity_hash INTEGER NOT NULL UNIQUE,
  resource_id INTEGER NOT NULL REFERENCES resources (id),
  name TEXT NOT NULL,
  -- gauge, updown, counter, or histogram.
  kind TEXT NOT NULL,
  -- cumulative or delta for a counter and a histogram. NULL for the rest.
  aggregation_temporality TEXT,
  unit TEXT NOT NULL,
  -- A JSON object.
  attributes TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS metric_series_name ON metric_series (name);

CREATE TABLE IF NOT EXISTS metric_points (
  metric_series_id INTEGER NOT NULL REFERENCES metric_series (id),
  -- Unix nanoseconds.
  recorded_at INTEGER NOT NULL,
  -- The sum of a histogram point.
  value REAL NOT NULL,
  -- The buckets of a histogram point as a JSON object: count, sum, min, max, and either
  -- bounds and counts, or scale, zero_count, positive, and negative for an exponential
  -- histogram. NULL for the other kinds.
  histogram TEXT,
  PRIMARY KEY (metric_series_id, recorded_at)
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS metric_minute_summaries (
  metric_series_id INTEGER NOT NULL REFERENCES metric_series (id),
  -- The start of the minute, in Unix nanoseconds.
  start_at INTEGER NOT NULL,
  point_count INTEGER NOT NULL,
  min_value REAL NOT NULL,
  max_value REAL NOT NULL,
  value_sum REAL NOT NULL,
  last_value REAL NOT NULL,
  -- How much a counter grew, whatever the aggregation temporality of its points. NULL for the
  -- other kinds.
  counter_increase REAL,
  -- The time the counter grew over. NULL for the other kinds.
  counter_increase_seconds REAL,
  -- The buckets of the points of a histogram merged into one, as metric_points.histogram has
  -- them. NULL for the other kinds.
  merged_histogram TEXT,
  PRIMARY KEY (metric_series_id, start_at)
) WITHOUT ROWID;

-- The columns are those of metric_minute_summaries, for an hour.
CREATE TABLE IF NOT EXISTS metric_hour_summaries (
  metric_series_id INTEGER NOT NULL REFERENCES metric_series (id),
  start_at INTEGER NOT NULL,
  point_count INTEGER NOT NULL,
  min_value REAL NOT NULL,
  max_value REAL NOT NULL,
  value_sum REAL NOT NULL,
  last_value REAL NOT NULL,
  counter_increase REAL,
  counter_increase_seconds REAL,
  merged_histogram TEXT,
  PRIMARY KEY (metric_series_id, start_at)
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS metric_summary_progress (
  -- metric_minute_summaries or metric_hour_summaries.
  summary_table TEXT NOT NULL PRIMARY KEY,
  -- Unix nanoseconds. Every minute, or hour, before it is summarized.
  summarized_until INTEGER NOT NULL
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS attribute_key_counts (
  -- The UTC date of the records.
  day TEXT NOT NULL,
  -- log, span, metric_series, or resource.
  attribute_owner TEXT NOT NULL,
  key TEXT NOT NULL,
  -- null, bool, int, float, string, array, or object, or mixed for more than one.
  json_type TEXT NOT NULL,
  -- How many records of the day have the key. A resource and a series count once a day.
  record_count INTEGER NOT NULL,
  -- 1 once the key has more distinct values on the day than attribute_value_counts keeps.
  has_more_values_than_listed INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (day, attribute_owner, key)
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS attribute_value_counts (
  -- The UTC date of the records.
  day TEXT NOT NULL,
  -- As in attribute_key_counts.
  attribute_owner TEXT NOT NULL,
  key TEXT NOT NULL,
  -- JSON.
  value TEXT NOT NULL,
  -- How many records of the day have the value.
  record_count INTEGER NOT NULL,
  PRIMARY KEY (day, attribute_owner, key, value)
) WITHOUT ROWID;

-- The indexer keeps up to 200 names a day, as attribute_value_counts keeps values.
CREATE TABLE IF NOT EXISTS span_name_counts (
  -- The UTC date of the spans.
  day TEXT NOT NULL,
  name TEXT NOT NULL,
  -- How many spans of the day have the name.
  record_count INTEGER NOT NULL,
  PRIMARY KEY (day, name)
) WITHOUT ROWID;

-- The indexer writes a row in the transaction of the rows of the frames up to it, so a crash
-- neither loses a frame nor indexes it twice.
CREATE TABLE IF NOT EXISTS indexed_journal_positions (
  -- logs, spans, or metrics.
  signal TEXT NOT NULL PRIMARY KEY,
  -- Hours since the Unix epoch: the UTC hour of the journal segment.
  segment_hour INTEGER NOT NULL,
  -- In the segment before compression. The frames before it are indexed.
  byte_offset INTEGER NOT NULL
) WITHOUT ROWID;
