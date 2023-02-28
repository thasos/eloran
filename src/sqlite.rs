use sqlx::pool::Pool;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use std::fs;
use std::path::Path;

pub async fn create_sqlite_pool() -> Pool<Sqlite> {
    SqlitePoolOptions::new()
        // TODO use const
        .connect(crate::DB_URL)
        .await
        .unwrap()
}

pub async fn init_database() {
    // create sqlite directory if needed
    let database_path = Path::new("sqlite");
    if !database_path.is_dir() {
        match fs::create_dir(database_path) {
            Ok(_) => (),
            Err(e) => println!(
                "failed to create {} : {}",
                database_path.to_string_lossy(),
                e
            ),
        }
    }

    // create sqlite database if needed
    if !Sqlite::database_exists(crate::DB_URL)
        .await
        .unwrap_or(false)
    {
        info!("creating database {}", crate::DB_URL);
        match Sqlite::create_database(crate::DB_URL).await {
            Ok(_) => info!("database successfully created"),
            Err(e) => error!("failed to create database {} : {}", crate::DB_URL, e),
        }
    } else {
        info!("database exists");
    }
    // tables
    // TODO check if already created ?
    let conn = SqlitePool::connect(crate::DB_URL).await.unwrap();
    let schema = r#"
CREATE TABLE IF NOT EXISTS users (
  id INTEGER PRIMARY KEY NOT NULL,
  password_hash TEXT NOT NULL,
  name TEXT NOT NULL,
  role TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS library (
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
CREATE TABLE IF NOT EXISTS core (
  id INTEGER PRIMARY KEY NOT NULL,
  last_successfull_scan_date INTEGER NOT NULL DEFAULT 0,
  last_successfull_extract_date INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS covers (
  id ULID PRIMARY KEY NOT NULL,
  cover BLOB DEFAULT NULL
);
    "#;
    match sqlx::query(schema).execute(&conn).await {
        Ok(_) => info!("tables successfully created"),
        Err(e) => error!("failed to create tables : {}", e),
    }
}

pub async fn init_users(db_url: &str) {
    let conn = SqlitePool::connect(db_url).await.unwrap();
    let schema = r#"
INSERT INTO users(id, password_hash, name, role)
VALUES (1,'pass123','admin','Admin'),
       (2,'666','thas','User');
    "#;
    match sqlx::query(schema).execute(&conn).await {
        Ok(_) => info!("users successfully created"),
        Err(e) => error!("failed to create users : {}", e),
    }
}
