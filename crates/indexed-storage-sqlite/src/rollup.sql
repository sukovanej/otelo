PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

-- resources and series are the tables of a day file, so one query reads both files.
CREATE TABLE IF NOT EXISTS resources (
  id INTEGER PRIMARY KEY,
  -- xxh3 of the service and the attributes.
  hash INTEGER NOT NULL UNIQUE,
  service TEXT NOT NULL,
  -- A JSON object.
  attributes TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS series (
  id INTEGER PRIMARY KEY,
  -- xxh3 of the resource, the name, the kind, the temporality, the unit, and the labels.
  hash INTEGER NOT NULL UNIQUE,
  resource_id INTEGER NOT NULL REFERENCES resources (id),
  name TEXT NOT NULL,
  -- gauge, updown, counter, or histogram.
  kind TEXT NOT NULL,
  -- delta for a counter and a histogram, whose summaries hold what each step added.
  -- NULL for the rest.
  temporality TEXT,
  unit TEXT NOT NULL,
  -- A JSON object.
  labels TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS series_name ON series (name);

CREATE TABLE IF NOT EXISTS minutes (
  series_id INTEGER NOT NULL REFERENCES series (id),
  -- The start of the minute, in Unix nanoseconds.
  start_at INTEGER NOT NULL,
  -- How many points the minute has. min, max, sum, and last are of their values.
  count INTEGER NOT NULL,
  min REAL NOT NULL,
  max REAL NOT NULL,
  sum REAL NOT NULL,
  last REAL NOT NULL,
  -- How much a counter grew. NULL for the other kinds.
  increase REAL,
  -- The time the counter grew over. NULL for the other kinds.
  seconds REAL,
  -- The merged buckets of a histogram, as points.histogram has them. NULL for the other
  -- kinds.
  histogram TEXT,
  PRIMARY KEY (series_id, start_at)
) WITHOUT ROWID;

-- The columns are those of minutes, for an hour.
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
  -- minutes or hours.
  rollup TEXT PRIMARY KEY,
  -- Unix nanoseconds. Every minute, or hour, before it is rolled up.
  rolled_until INTEGER NOT NULL
) WITHOUT ROWID;
