PRAGMA journal_mode = WAL;

CREATE TABLE IF NOT EXISTS telemetry_indexes (
  -- logs or spans.
  signal TEXT NOT NULL,
  key TEXT NOT NULL,
  PRIMARY KEY (signal, key)
) WITHOUT ROWID;

-- otelo has one password, so the table has at most one row.
CREATE TABLE IF NOT EXISTS passwords (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  -- The SHA-256 of the password.
  hash BLOB NOT NULL
);
