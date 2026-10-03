-- otelo has one password, so the table has at most one row.
CREATE TABLE passwords (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  -- The SHA-256 of the password.
  hash BLOB NOT NULL
);
