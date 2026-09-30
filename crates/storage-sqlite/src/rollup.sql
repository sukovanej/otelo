PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

-- resources and series are the tables of a day file, so one query reads both files.
CREATE TABLE IF NOT EXISTS resources (
  id INTEGER PRIMARY KEY,
  hash INTEGER NOT NULL UNIQUE,
  service TEXT NOT NULL,
  attributes TEXT NOT NULL
);

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

CREATE TABLE IF NOT EXISTS minutes (
  series_id INTEGER NOT NULL REFERENCES series (id),
  start_at INTEGER NOT NULL,
  count INTEGER NOT NULL,
  min REAL NOT NULL,
  max REAL NOT NULL,
  sum REAL NOT NULL,
  last REAL NOT NULL,
  increase REAL,
  seconds REAL,
  histogram TEXT,
  PRIMARY KEY (series_id, start_at)
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS hours (
  series_id INTEGER NOT NULL REFERENCES series (id),
  start_at INTEGER NOT NULL,
  count INTEGER NOT NULL,
  min REAL NOT NULL,
  max REAL NOT NULL,
  sum REAL NOT NULL,
  last REAL NOT NULL,
  increase REAL,
  seconds REAL,
  histogram TEXT,
  PRIMARY KEY (series_id, start_at)
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS cursors (
  rollup TEXT PRIMARY KEY,
  rolled_until INTEGER NOT NULL
) WITHOUT ROWID;
