PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

-- The resources and the series have the shape of a day file, so a query reads
-- the rollups as it reads the raw points.
CREATE TABLE IF NOT EXISTS resources (
  id INTEGER PRIMARY KEY,
  hash INTEGER NOT NULL UNIQUE,
  service TEXT NOT NULL,
  attributes TEXT NOT NULL
);

-- A rolled up counter or histogram holds what each step added, so its
-- temporality is delta.
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

-- One row per series and minute that has points. count, min, max, sum, and
-- last sum up the values of the points. increase and seconds are how much a
-- counter grew and over what time. histogram is the merged buckets of a
-- histogram as JSON, in the shape of points.histogram.
CREATE TABLE IF NOT EXISTS minutes (
  series_id INTEGER NOT NULL REFERENCES series (id),
  start INTEGER NOT NULL,
  count INTEGER NOT NULL,
  min REAL NOT NULL,
  max REAL NOT NULL,
  sum REAL NOT NULL,
  last REAL NOT NULL,
  increase REAL,
  seconds REAL,
  histogram TEXT,
  PRIMARY KEY (series_id, start)
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS hours (
  series_id INTEGER NOT NULL REFERENCES series (id),
  start INTEGER NOT NULL,
  count INTEGER NOT NULL,
  min REAL NOT NULL,
  max REAL NOT NULL,
  sum REAL NOT NULL,
  last REAL NOT NULL,
  increase REAL,
  seconds REAL,
  histogram TEXT,
  PRIMARY KEY (series_id, start)
) WITHOUT ROWID;

-- Every minute, or hour, before rolled_until is rolled up.
CREATE TABLE IF NOT EXISTS cursors (
  rollup TEXT PRIMARY KEY,
  rolled_until INTEGER NOT NULL
) WITHOUT ROWID;
