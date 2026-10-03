-- A file from before the migrations has this table at user_version 0.
CREATE TABLE IF NOT EXISTS telemetry_indexes (
  -- logs or spans.
  signal TEXT NOT NULL,
  key TEXT NOT NULL,
  PRIMARY KEY (signal, key)
) WITHOUT ROWID;
