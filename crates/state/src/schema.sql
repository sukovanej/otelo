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

CREATE TABLE IF NOT EXISTS dashboards (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT NOT NULL,
  -- A JSON array of the widgets, in their order.
  widgets TEXT NOT NULL,
  -- Unix nanoseconds.
  created_at INTEGER NOT NULL,
  -- Unix nanoseconds.
  updated_at INTEGER NOT NULL
);
