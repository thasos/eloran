# Eloran

## Database Schemas

```
CREATE TABLE users (
  id INTEGER PRIMARY KEY NOT NULL,
  password_hash TEXT NOT NULL,
  name TEXT NOT NULL,
  role TEXT NOT NULL
);
CREATE TABLE library (
  id ULID PRIMARY KEY NOT NULL,
  filename TEXT NOT NULL,
  parent_path TEXT NOT NULL,
  read_status BOOLEAN DEFAULT FALSE,
  scan_me BOOLEAN DEFAULT TRUE,
  added_date INTEGER NOT NULL,
  file_type TEXT DEFAULT NULL,
  size INTEGER NOT NULL DEFAULT 0,
  total_pages INTEGER NOT NULL DEFAULT 0,
  current_page INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE core (
  id INTEGER PRIMARY KEY NOT NULL,
  last_successfull_scan_date INTEGER NOT NULL DEFAULT 0,
  last_successfull_extract_date INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE covers (
  id ULID PRIMARY KEY NOT NULL,
  cover BLOB DEFAULT NULL
);
```

u64 for a timestamp : year 33658...
