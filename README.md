# Eloran

## Database Schemas

```
CREATE TABLE users (
  id int primary key not null,
  password_hash text not null,
  name text not null,
  role text not null
);
CREATE TABLE library (
  id ulid primary key not null,
  filename text not null,
  path text not null,
  read_status text,
  scan_me text,
  added_date int,
  file_type text,
  size int,
  total_pages int,
  current_page int
);
```
