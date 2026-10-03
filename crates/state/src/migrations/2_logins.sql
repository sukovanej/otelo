-- otelo has one password, so the table has at most one row.
CREATE TABLE passwords (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  -- An argon2id PHC string.
  hash TEXT NOT NULL
);

CREATE TABLE sessions (
  -- The SHA-256 of the token.
  token_hash BLOB PRIMARY KEY,
  -- Unix nanoseconds.
  created_at INTEGER NOT NULL,
  -- Unix nanoseconds.
  last_used_at INTEGER NOT NULL
) WITHOUT ROWID;
