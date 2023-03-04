use sqlx::pool::Pool;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Row;
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
    // TODO rename filename into name ? (same for directories)
    let schema = r#"
CREATE TABLE IF NOT EXISTS users (
  id INTEGER PRIMARY KEY NOT NULL,
  password_hash TEXT NOT NULL,
  name TEXT NOT NULL,
  role TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS directories (
  id ULID PRIMARY KEY NOT NULL,
  directory_name TEXT NOT NULL,
  parent_path TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS files (
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
  library_path TEXT DEFAULT NULL,
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

/// register the library path in database if needed
pub async fn set_library_path(library_path: &Path, conn: &Pool<Sqlite>) {
    // TODO test in exsists to avoid a useless write...
    let insert_library_path = format!(
        "INSERT OR IGNORE INTO core(id, library_path) VALUES (1,'{}');",
        library_path.to_string_lossy().replace('\'', "''")
    );
    match sqlx::query(&insert_library_path).execute(conn).await {
        Ok(_) => info!("library path successfully created"),
        Err(e) => error!("failed to create library path : {}", e),
    }
}

/// retrieve the library path in database
pub async fn get_library_path(conn: &Pool<Sqlite>) -> String {
    match sqlx::query("SELECT library_path FROM core WHERE id = 1;")
        .fetch_one(conn)
        .await
    {
        Ok(library_path) => {
            let library_path: String = library_path.try_get("library_path").unwrap();
            library_path
        }
        Err(e) => {
            error!("failed to get library path : {}", e);
            String::new()
        }
    }
}
