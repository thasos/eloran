// TODO use `?` and `bind()` everywhere (see insert_cover)
use crate::scanner::FileInfo;

use base64::{engine::general_purpose, Engine as _};
use image::{DynamicImage, ImageOutputFormat};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use sqlx::{pool::Pool, Row};
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::process;
use std::time::Duration;

pub async fn create_sqlite_pool() -> Pool<Sqlite> {
    match SqlitePoolOptions::new()
        .max_lifetime(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(5))
        // TODO use const
        .connect(crate::DB_URL)
        .await
    {
        Ok(pool) => pool,
        Err(e) => {
            error!("can't create pool connection : {e}");
            process::exit(255);
        }
    }
}

pub async fn init_database() {
    // create sqlite directory if needed
    let database_path = Path::new("sqlite");
    if !database_path.is_dir() {
        match fs::create_dir(database_path) {
            Ok(_) => (),
            Err(e) => error!(
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
    // TODO use after connect ?
    // https://docs.rs/sqlx/0.6.2/sqlx/pool/struct.PoolOptions.html#method.after_connect
    // ou `create_if_missing`
    // https://github.com/launchbadge/sqlx/issues/1114
    let conn = SqlitePool::connect(crate::DB_URL).await.unwrap();
    let schema = r#"
CREATE TABLE IF NOT EXISTS users (
  id INTEGER PRIMARY KEY NOT NULL,
  password_hash TEXT NOT NULL,
  name TEXT NOT NULL,
  role TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS directories (
  id ULID PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  parent_path TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS files (
  id ULID PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  parent_path TEXT NOT NULL,
  read_status BOOLEAN DEFAULT FALSE,
  scan_me BOOLEAN DEFAULT TRUE,
  added_date INTEGER NOT NULL,
  format TEXT DEFAULT NULL,
  size INTEGER NOT NULL DEFAULT 0,
  total_pages INTEGER NOT NULL DEFAULT 0,
  current_page INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS covers (
  id ULID PRIMARY KEY NOT NULL,
  cover BLOB DEFAULT NULL
);
CREATE TABLE IF NOT EXISTS core (
  id INTEGER PRIMARY KEY NOT NULL,
  library_path TEXT DEFAULT NULL,
  last_successfull_scan_date INTEGER NOT NULL DEFAULT 0,
  last_successfull_extract_date INTEGER NOT NULL DEFAULT 0
);
    "#;
    match sqlx::query(schema).execute(&conn).await {
        Ok(_) => info!("tables successfully created"),
        Err(e) => error!("failed to create tables : {}", e),
    };
    conn.close().await;
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
    let path_in_base = get_library_path(conn).await;
    if path_in_base.is_empty() {
        let insert_library_path = format!(
            "INSERT OR IGNORE INTO core(id, library_path) VALUES (1,'{}');",
            library_path.to_string_lossy().replace('\'', "''")
        );
        match sqlx::query(&insert_library_path).execute(conn).await {
            Ok(_) => info!("library path successfully created"),
            Err(e) => error!("failed to create library path : {}", e),
        }
    // }
    } else {
        // TODO test si les path sont =
        if path_in_base != library_path.to_string_lossy() {
            error!("library path changed ! I need to purge database and recreate it from scratch");
            match sqlx::query("DELETE FROM core ; DELETE FROM files ; DELETE FROM directories ;")
                .execute(conn)
                .await
            {
                Ok(_) => info!("database successfully purged"),
                Err(e) => error!("failed to purge database : {}", e),
            }
            init_database().await;
        }
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

/// get FileInfo from file path
// path = `/fantasy/The Witcher/Sorceleur - L'Integrale - Andrzej Sapkowski.epub`
pub async fn _get_files_from_path(file_path: &str, conn: &Pool<Sqlite>) -> FileInfo {
    // separe parent_path and filename
    let mut path_elements: Vec<&str> = file_path.split('/').collect();
    let file_name = match path_elements.last() {
        Some(file_name) => *file_name,
        None => "",
    };
    // remove file name (after last '/')
    path_elements.pop();
    let mut parent_path = get_library_path(conn).await;
    for element in path_elements {
        parent_path.push_str(element);
        parent_path.push('/');
    }
    // remove last '/'
    parent_path.pop();

    let file: FileInfo = match sqlx::query_as(&format!(
        "SELECT * FROM files WHERE parent_path = '{}' AND name = '{}';",
        parent_path.replace('\'', "''"),
        file_name.replace('\'', "''"),
    ))
    .fetch_one(conn)
    .await
    {
        Ok(file_found) => file_found,
        Err(e) => {
            error!("unable to retrieve file infos from database : {}", e);
            FileInfo::new()
        }
    };
    file
}

/// get FileInfo from file id
pub async fn get_files_from_id(id: &str, conn: &Pool<Sqlite>) -> FileInfo {
    // 🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥
    // TODO ⚠️ select all but cover ⚠️  no need it in memory...
    // 🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥
    let file: FileInfo = match sqlx::query_as(&format!("SELECT * FROM files WHERE id = '{}';", id,))
        .fetch_one(conn)
        .await
    {
        Ok(file_found) => file_found,
        Err(e) => {
            error!("unable to retrieve file infos from database : {}", e);
            FileInfo::new()
        }
    };
    file
}

/// get currentPage from file id (can be usefull for sync)
pub async fn _get_current_page_from_id(id: &str, conn: &Pool<Sqlite>) -> i32 {
    let file: i32 = match sqlx::query(&format!(
        "SELECT current_page FROM files WHERE id = '{}';",
        id,
    ))
    .fetch_one(conn)
    .await
    {
        Ok(file_found) => file_found.get("current_page"),
        Err(e) => {
            error!(
                "unable to retrieve current page from database fore id {} : {}",
                id, e
            );
            0
        }
    };
    file
}

/// set currentPage from file id
pub async fn set_current_page_from_id(id: &str, page: &i32, conn: &Pool<Sqlite>) {
    match sqlx::query(&format!(
        "UPDATE files SET current_page = '{}' WHERE id = '{}';",
        page, id,
    ))
    .execute(conn)
    .await
    {
        Ok(_) => debug!("current_page successfully setted to {} for id {}", page, id),
        Err(e) => {
            error!(
                "unable to set current page from database fore id {} : {}",
                id, e
            );
        }
    };
}

// TODO easy test here
pub fn image_to_base64(img: &DynamicImage) -> String {
    let mut image_data: Vec<u8> = Vec::new();
    img.write_to(
        &mut Cursor::new(&mut image_data),
        ImageOutputFormat::Jpeg(75),
    )
    .unwrap();
    general_purpose::STANDARD.encode(image_data)
}

/// insert cover for a file
pub async fn insert_cover(file: &FileInfo, cover: &Vec<u8>, conn: &Pool<Sqlite>) {
    match sqlx::query("INSERT OR REPLACE INTO covers(id,cover) VALUES (?, ?);")
        .bind(&file.id)
        .bind(cover)
        .execute(conn)
        .await
    {
        Ok(_) => debug!("cover updated for file {}/{}", file.parent_path, file.name),
        Err(e) => error!(
            "failed to update covers for file {}/{} : {e}",
            file.parent_path, file.name
        ),
    };
}

/// insert total_pages for a file
pub async fn insert_total_pages(file: &FileInfo, total_pages: i32, conn: &Pool<Sqlite>) {
    match sqlx::query(&format!(
        "UPDATE files SET total_pages = '{}' WHERE id = '{}';",
        total_pages, file.id
    ))
    .execute(conn)
    .await
    {
        Ok(_) => debug!(
            "total_pages updated for file {}/{}",
            file.parent_path, file.name
        ),
        Err(e) => error!(
            "failed to update total_pages for file {}/{} : {e}",
            file.parent_path, file.name
        ),
    };
}

/// get cover from id, raw (Vec<u8>)
pub async fn get_cover_from_id(file: &FileInfo, conn: &Pool<Sqlite>) -> Option<Vec<u8>> {
    match sqlx::query("SELECT cover FROM covers WHERE id = ?;")
        .bind(&file.id)
        .fetch_one(conn)
        .await
    {
        Ok(cover) => Some(cover.get("cover")),
        Err(e) => {
            warn!(
                "failed to get cover for file {}/{} : {e}",
                file.parent_path, file.name
            );
            None
        }
    }
}
