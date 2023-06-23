use crate::http_server::User;
use crate::scanner::{DirectoryInfo, FileInfo, Library};

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use sqlx::{pool::Pool, Row};
use std::fs;
use std::path::Path;
use std::process;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use ulid::Ulid;

pub async fn create_sqlite_pool() -> Pool<Sqlite> {
    match SqlitePoolOptions::new()
        .max_lifetime(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(5))
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
  name TEXT NOT NULL UNIQUE,
  role TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS directories (
  id ULID PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  parent_path TEXT NOT NULL,
  file_number INTEGER DEFAULT NULL
);
CREATE TABLE IF NOT EXISTS files (
  id ULID PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  library_name TEXT NOT NULL,
  parent_path TEXT NOT NULL,
  scan_me BOOLEAN DEFAULT TRUE,
  added_date INTEGER NOT NULL,
  format TEXT DEFAULT NULL,
  size INTEGER NOT NULL DEFAULT 0,
  total_pages INTEGER NOT NULL DEFAULT 0,
  read_by TEXT DEFAULT NULL,
  bookmarked_by TEXT DEFAULT NULL
);
CREATE TABLE IF NOT EXISTS covers (
  id ULID PRIMARY KEY NOT NULL,
  cover BLOB DEFAULT NULL
);
CREATE TABLE IF NOT EXISTS reading (
  id INTEGER PRIMARY KEY NOT NULL,
  file_id ULID NOT NULL,
  user_id INTEGER NOT NULL,
  page INTEGER NOT NULL,
  UNIQUE(file_id, user_id)
);
CREATE TABLE IF NOT EXISTS core (
  id INTEGER PRIMARY KEY NOT NULL,
  name TEXT DEFAULT NULL UNIQUE,
  path TEXT DEFAULT NULL UNIQUE,
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

// TODO delete this when install page will be done
pub async fn init_default_users() {
    let conn = SqlitePool::connect(crate::DB_URL).await.unwrap();
    let schema = r#"
INSERT OR IGNORE INTO users(id, password_hash, name, role)
VALUES (1,'pass123','admin','Admin'),
       (2,'666','thas','User'),
       (3,'666','dod','User'),
       (4,'666','swiip','User');
    "#;
    match sqlx::query(schema).execute(&conn).await {
        Ok(_) => info!("users successfully created"),
        Err(e) => error!("failed to create users : {}", e),
    }
}

/// register the library path in database if needed
pub async fn create_library_path(library_path: Vec<String>) {
    // TODO test if path exists before add
    for path in library_path {
        let library = Library {
            id: 0,
            name: path
                .trim_end_matches('/')
                .split('/')
                .last()
                .unwrap()
                .to_string(),
            path: path.to_string(),
            last_successfull_scan_date: 0,
            last_successfull_extract_date: 0,
        };
        debug!("set library path : {path}");
        let conn = SqlitePool::connect(crate::DB_URL).await.unwrap();
        // ignore UNIQUE constraint when insert here (or add a test "if exists" ?)
        match sqlx::query("INSERT OR IGNORE INTO core(name, path) VALUES (?, ?);")
            .bind(library.name)
            .bind(library.path)
            .execute(&conn)
            .await
        {
            Ok(_) => info!("library path successfully created : {path}"),
            Err(e) => error!("failed to create library path {path} : {}", e),
        }
    }
}

/// delete a library path in database
pub async fn delete_library_from_id(library_list: &Vec<Library>, conn: &Pool<Sqlite>) {
    for library in library_list {
        debug!("delete library id {} : {}", library.id, library.name);
        match sqlx::query("DELETE FROM core WHERE id = ?;")
            .bind(library.id)
            .execute(conn)
            .await
        {
            Ok(_) => info!("library {} successfully deleted", library.name),
            Err(e) => error!("failed to delete library {} : {}", library.name, e),
        }
    }
}

/// delete files of a library name
pub async fn delete_files_from_library(library_list: &Vec<Library>, conn: &Pool<Sqlite>) {
    for library in library_list {
        debug!("delete files from library {}", library.name);
        match sqlx::query("DELETE FROM files WHERE library_name = ?;")
            .bind(library.name.clone())
            .execute(conn)
            .await
        {
            Ok(_) => info!("files of library {} successfully deleted", library.name),
            Err(e) => error!("failed to delete files of library {} : {}", library.name, e),
        }
    }
}

/// retrieve all the library path in database
/// we can specify a name, in this case, return a Vec with one row
pub async fn get_library(
    name: Option<&str>,
    id: Option<&str>,
    conn: &Pool<Sqlite>,
) -> Vec<Library> {
    // add a WHERE condition when a name is given
    let where_clause = if name.is_some() {
        match name {
            Some(name) => format!("WHERE name = '{}'", name),
            None => "".to_string(),
        }
    } else if id.is_some() {
        match id {
            Some(id) => format!("WHERE id = '{}'", id),
            None => "".to_string(),
        }
    } else {
        "".to_string()
    };
    // send query
    match sqlx::query_as(&format!("SELECT * FROM core {};", where_clause))
        .fetch_all(conn)
        .await
    {
        Ok(library_path_rows) => library_path_rows,
        Err(e) => {
            error!("failed to get library path : {}", e);
            Vec::with_capacity(0)
        }
    }
    // TODO add a test : should return only 1 row when a name is given ?
}

/// get FileInfo from file path
pub async fn _get_files_from_path(file_path: &str, conn: &Pool<Sqlite>) -> FileInfo {
    // separe parent_path and filename
    let mut path_elements: Vec<&str> = file_path.split('/').collect();
    let file_name = match path_elements.last() {
        Some(file_name) => *file_name,
        None => "",
    };
    // remove file name (after last '/')
    path_elements.pop();
    let parent_path = get_library(None, None, conn).await;
    let mut parent_path = parent_path.first().unwrap().name.to_owned();
    for element in path_elements {
        parent_path.push_str(element);
        parent_path.push('/');
    }
    // remove last '/'
    parent_path.pop();

    let file: FileInfo =
        match sqlx::query_as("SELECT * FROM files WHERE parent_path = ? AND name = ?;")
            .bind(parent_path)
            .bind(file_name)
            .fetch_one(conn)
            .await
        {
            Ok(file_found) => file_found,
            Err(e) => {
                error!(
                    "unable to retrieve file infos from path from database : {}",
                    e
                );
                FileInfo::new()
            }
        };
    file
}

/// get FileInfo from file id
pub async fn get_files_from_file_id(file_id: &str, conn: &Pool<Sqlite>) -> FileInfo {
    let file: FileInfo = match sqlx::query_as("SELECT * FROM files WHERE id = ?;")
        .bind(file_id)
        .fetch_one(conn)
        .await
    {
        Ok(file_found) => file_found,
        Err(e) => {
            error!(
                "unable to retrieve file infos from id from database : {}",
                e
            );
            FileInfo::new()
        }
    };
    file
}

/// get reading FileInfo from user id
pub async fn get_reading_files_from_user_id(user_id: &i64, conn: &Pool<Sqlite>) -> Vec<FileInfo> {
    let file: Vec<FileInfo> = match sqlx::query_as(
        "SELECT files.* FROM reading
        INNER JOIN files ON files.id = reading.file_id
        WHERE reading.user_id = ?;",
    )
    .bind(user_id)
    .fetch_all(conn)
    .await
    {
        Ok(file_found) => file_found,
        Err(e) => {
            error!("unable to retrieve reading file : {e}");
            Vec::with_capacity(0)
        }
    };
    file
}

/// get currentPage from file id (can be usefull for sync)
pub async fn get_current_page_from_file_id(
    user_id: i64,
    file_id: &str,
    conn: &Pool<Sqlite>,
) -> i32 {
    let page_number: i32 =
        match sqlx::query("SELECT page FROM reading WHERE file_id = ? AND user_id = ?;")
            .bind(file_id)
            .bind(user_id)
            .fetch_one(conn)
            .await
        {
            Ok(file_found) => file_found.get("page"),
            // set page to 0 if not set
            Err(_) => 0,
        };
    page_number
}

/// set currentPage from file id
pub async fn remove_file_id_from_reading(file_id: &str, user_id: &i64, conn: &Pool<Sqlite>) {
    match sqlx::query("DELETE FROM reading WHERE file_id = ? AND user_id = ?;")
        .bind(file_id)
        .bind(user_id)
        .execute(conn)
        .await
    {
        Ok(_) => debug!("file id {file_id} removed from reading table"),
        Err(e) => error!("unable to remove file id {file_id} from reading table : {e}"),
    };
}

/// set currentPage from file id
pub async fn set_current_page_for_file_id(
    file_id: &str,
    user_id: &i64,
    page: &i32,
    conn: &Pool<Sqlite>,
) {
    match sqlx::query("INSERT OR REPLACE INTO reading(file_id,user_id,page) VALUES (?, ?, ?);")
        .bind(file_id)
        .bind(user_id)
        .bind(page)
        .execute(conn)
        .await
    {
        Ok(_) => debug!(
            "current_page successfully setted to {} for id {}",
            page, file_id
        ),
        Err(e) => {
            error!(
                "unable to set current page from database fore id {} : {}",
                file_id, e
            );
        }
    };
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
    match sqlx::query("UPDATE files SET total_pages = ? WHERE id = ?;")
        .bind(total_pages)
        .bind(&file.id)
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

/// set scan_me flag
pub async fn set_scan_flag(file: &FileInfo, flag: i8, conn: &Pool<Sqlite>) {
    match sqlx::query("UPDATE files SET scan_me = ? WHERE id = ?;")
        .bind(flag)
        .bind(&file.id)
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

// pub async fn get_user(name: Option<&str>, id: Option<&str>, conn: &Pool<Sqlite>) -> i32 {
pub async fn get_user(name: Option<&str>, id: Option<&str>, conn: &Pool<Sqlite>) -> Vec<User> {
    // TODO optional WHERE ?
    let where_clause = if name.is_some() {
        match name {
            Some(name) => format!("WHERE name = '{}'", name),
            None => "".to_string(),
        }
    } else if id.is_some() {
        match id {
            Some(id) => format!("WHERE id = '{}'", id),
            None => "".to_string(),
        }
    } else {
        "".to_string()
    };
    // let user_id: i32 = match sqlx::query(&format!("SELECT id FROM users {};", where_clause))
    match sqlx::query_as(&format!("SELECT * FROM users {};", where_clause))
        .fetch_all(conn)
        .await
    {
        // Ok(id) => id.get("id"),
        Ok(user) => user,
        Err(e) => {
            error!("failed to get user : {e}");
            Vec::with_capacity(0)
        }
    }
}

pub async fn set_flag_status(
    // TODO use flag to standardise this fn
    flag: &str,
    user_id: i64,
    file_id: &str,
    conn: &Pool<Sqlite>,
) -> bool {
    // prepare sql queries
    let (flag_field, select_query, toggle_query) = if flag == "bookmark" {
        let flag_field = "bookmarked_by";
        let select_query = "SELECT bookmarked_by FROM files WHERE id = ?;";
        let toggle_query = "UPDATE files SET bookmarked_by = ? WHERE id = ?;";
        (flag_field, select_query, toggle_query)
    } else if flag == "read_status" {
        let flag_field = "read_by";
        let select_query = "SELECT read_by FROM files WHERE id = ?;";
        let toggle_query = "UPDATE files SET read_by = ? WHERE id = ?;";
        (flag_field, select_query, toggle_query)
    } else {
        let flag_field = "";
        let select_query = "";
        let toggle_query = "";
        (flag_field, select_query, toggle_query)
    };

    // retrieve fav_list for user
    match sqlx::query(select_query)
        .bind(file_id)
        .fetch_one(conn)
        .await
    {
        Ok(user_list) => {
            let flag_status: bool;
            let user_list_string: String = user_list.get(flag_field);
            let updated_user_list = if user_list_string.is_empty() {
                flag_status = true;
                user_id.to_string()
            } else {
                // create new list
                // String `1,2,3,...` to Vec `[1, 2, 3, ...]`
                let mut user_list_vec: Vec<String> =
                    user_list_string.split(',').map(|x| x.to_string()).collect();
                // insert or remove user form list
                if let Ok(found_user_index) = user_list_vec.binary_search(&user_id.to_string()) {
                    flag_status = false;
                    user_list_vec.remove(found_user_index);
                } else {
                    flag_status = true;
                    user_list_vec.push(user_id.to_string());
                }
                // Vec `[1, 2, 3, ...]` to String `1,2,3,...`
                user_list_vec.join(",")
            };
            // set status
            match sqlx::query(toggle_query)
                .bind(updated_user_list)
                .bind(file_id)
                .execute(conn)
                .await
            {
                Ok(_) => debug!("flag {} {} added to user {}", flag, file_id, user_id),
                Err(e) => error!(
                    "failed to add flag {} {} to user {} : {e}",
                    flag, file_id, user_id,
                ),
            };
            flag_status
        }
        Err(e) => {
            error!(
                "failed to add {} {} to user {} : {e}",
                flag, file_id, user_id,
            );
            false
        }
    }
}

pub async fn get_flag_status(flag: &str, user_id: i64, file_id: &str, conn: &Pool<Sqlite>) -> bool {
    let (request, column) = match flag {
        "bookmark" => (
            "SELECT bookmarked_by FROM files WHERE id = ?;",
            "bookmarked_by",
        ),
        "read_status" => ("SELECT read_by FROM files WHERE id = ?;", "read_by"),
        _ => ("", ""),
    };
    match sqlx::query(request).bind(file_id).fetch_one(conn).await {
        Ok(user_list) => {
            let user_list: String = user_list.get(column);
            user_list.contains(&user_id.to_string())
        }
        Err(e) => {
            error!("unable to retrieve flag for file {file_id} : {e}");
            false
        }
    }
}

pub async fn bookmarks_for_user_id(id: i64, conn: &Pool<Sqlite>) -> Vec<FileInfo> {
    let request = format!("SELECT * FROM files WHERE bookmarked_by LIKE '%{}%';", id);
    let results: Vec<FileInfo> = match sqlx::query_as(&request).fetch_all(conn).await {
        Ok(files_list) => files_list,
        Err(e) => {
            error!(
                "unable to find bookmarked files in database for user id {}: {e}",
                id
            );
            Vec::with_capacity(0)
        }
    };
    results
}

pub async fn search_file_from_string(search_query: &str, conn: &Pool<Sqlite>) -> Vec<FileInfo> {
    let request = format!(
        "SELECT * FROM files WHERE name LIKE '%{}%' OR parent_path LIKE '%{}%';",
        search_query, search_query
    );
    let results: Vec<FileInfo> = match sqlx::query_as(&request).fetch_all(conn).await {
        Ok(files_list) => files_list,
        Err(e) => {
            error!("unable to search files in database : {e}");
            Vec::with_capacity(0)
        }
    };
    results
}

pub async fn search_directory_from_string(
    search_query: &str,
    conn: &Pool<Sqlite>,
) -> Vec<DirectoryInfo> {
    let request = format!(
        "SELECT * FROM directories WHERE name LIKE '%{}%' OR parent_path LIKE '%{}%';",
        search_query, search_query
    );
    let results: Vec<DirectoryInfo> = match sqlx::query_as(&request).fetch_all(conn).await {
        Ok(directories_list) => directories_list,
        Err(e) => {
            error!("unable to search directories in database : {e}");
            Vec::with_capacity(0)
        }
    };
    results
}

/// get all file in a directory path from database
pub async fn get_files_from_directory(
    parent_path: &str,
    directory_name: &str,
    conn: &Pool<Sqlite>,
) -> Vec<FileInfo> {
    // WHY here we need to replace ' with '' in sqlite query ???
    let files: Vec<FileInfo> = match sqlx::query_as(&format!(
        "SELECT * FROM files WHERE parent_path = '{}/{}'",
        parent_path.replace('\'', "''"),
        directory_name.replace('\'', "''")
    ))
    .fetch_all(conn)
    .await
    {
        Ok(file_found) => file_found,
        Err(e) => {
            error!("unable to retrieve file infos from database : {}", e);
            let empty_list: Vec<FileInfo> = Vec::new();
            empty_list
        }
    };
    files
}

/// get last successfull scan date in EPOCH format from database
pub async fn get_last_successfull_scan_date(library_path: i64, conn: &Pool<Sqlite>) -> Duration {
    let last_successfull_scan_date: i64 = match sqlx::query(
        "SELECT last_successfull_scan_date FROM core WHERE id = ?",
    )
    .bind(library_path)
    .fetch_one(conn)
    .await
    {
        Ok(epoch_date_row) => {
            let epoch_date: i64 = epoch_date_row
                .try_get("last_successfull_scan_date")
                .unwrap();
            // TODO pretty display of epoch time
            info!("last successfull scan date : {}", &epoch_date);
            epoch_date
        }
        Err(_) => {
            warn!("could not found last successfull scan date, I will perform a full scan, be patient");
            0
        }
    };
    Duration::from_secs(u64::try_from(last_successfull_scan_date).unwrap())
}

/// when a new file is found or uploaded, insert all values found
/// return file id
pub async fn insert_new_file(file: &mut FileInfo, ulid: Option<&str>, conn: &Pool<Sqlite>) {
    // generate ulid if needed
    let ulid = match ulid {
        Some(ulid) => ulid.to_string(),
        None => Ulid::new().to_string(),
    };
    file.id = ulid;
    match sqlx::query(
        "INSERT OR REPLACE INTO files(id, name, library_name, parent_path, size, added_date, scan_me, format, total_pages, read_by, bookmarked_by)
                    VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);")
        .bind(&file.id)
        .bind(&file.name)
        .bind(&file.library_name)
        .bind(&file.parent_path)
        .bind(file.size)
        .bind(file.added_date)
        .bind(file.scan_me)
        .bind(&file.format)
        .bind(file.total_pages)
        .bind(&file.read_by)
        .bind(&file.bookmarked_by)
        .execute(conn).await {
        Ok(_) => {
            debug!("file insertion successfull ({}/{})", &file.parent_path, &file.id)
        }
        Err(e) => error!("file insertion failed ({}/{}) : {e}", &file.parent_path, &file.id),
    };
}

/// delete a file in database
pub async fn delete_file(file: &FileInfo, conn: &Pool<Sqlite>) {
    match sqlx::query("DELETE FROM files WHERE name = ? AND parent_path = ?;")
        .bind(&file.name)
        .bind(&file.parent_path)
        .execute(conn)
        .await
    {
        Ok(_) => {
            info!("file {}/{} deleted", file.name, file.parent_path)
        }
        Err(e) => error!("delete ko : {}", e),
    }
}

/// get all diretories in a path from database
pub async fn get_registered_directories(conn: &Pool<Sqlite>) -> Vec<DirectoryInfo> {
    let registered_directories: Vec<DirectoryInfo> =
        match sqlx::query_as("SELECT * FROM directories ;")
            .fetch_all(conn)
            .await
        {
            Ok(file_found) => file_found,
            Err(e) => {
                error!("unable to retrieve directories from database : {}", e);
                let empty_list: Vec<DirectoryInfo> = Vec::new();
                empty_list
            }
        };
    registered_directories
}

/// delete a directory in database
pub async fn delete_directory(directory: &DirectoryInfo, conn: &Pool<Sqlite>) {
    match sqlx::query(
        "DELETE FROM directories WHERE name = ? AND parent_path = ?;
         DELETE FROM files WHERE parent_path = ?;",
    )
    .bind(&directory.name)
    .bind(&directory.parent_path)
    .bind(format!("{}/{}", &directory.parent_path, &directory.name))
    .execute(conn)
    .await
    {
        Ok(_) => {
            info!(
                "directory {}/{} deleted",
                directory.name, directory.parent_path
            )
        }
        Err(e) => error!("unable to delete directory in database : {e}"),
    }
}

/// return a directory if it exists in database
pub async fn check_if_directory_exists(
    parent_path: &str,
    directory_name: &str,
    conn: &Pool<Sqlite>,
) -> Vec<DirectoryInfo> {
    let directory_found: Vec<DirectoryInfo> =
        match sqlx::query_as("SELECT * FROM directories WHERE name = ? AND parent_path = ?;")
            .bind(directory_name)
            .bind(parent_path)
            .fetch_all(conn)
            .await
        {
            Ok(dir_found) => dir_found,
            Err(e) => {
                error!("unable to check if directory exists in database : {}", e);
                let empty_list: Vec<DirectoryInfo> = Vec::new();
                empty_list
            }
        };
    directory_found
}

/// return a file if it exists in database
pub async fn check_if_file_exists(
    parent_path: &str,
    filename: &str,
    conn: &Pool<Sqlite>,
) -> Vec<FileInfo> {
    let file_found: Vec<FileInfo> =
        match sqlx::query_as("SELECT * FROM files WHERE name = ? AND parent_path = ?;")
            .bind(filename)
            .bind(parent_path)
            .fetch_all(conn)
            .await
        {
            Ok(file_found) => file_found,
            Err(e) => {
                error!("unable to check if file exists in database : {}", e);
                let empty_list: Vec<FileInfo> = Vec::new();
                empty_list
            }
        };
    file_found
}

/// update last successfull scan date in EPOCH format in database
pub async fn update_last_successfull_scan_date(library_path: &i64, conn: &Pool<Sqlite>) {
    // le at_least_one_insert_or_delete est pas bon car si rien change, c'est ok
    let now = SystemTime::now();
    let since_the_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
    match sqlx::query("UPDATE core SET last_successfull_scan_date = ? WHERE id = ?;")
        .bind(since_the_epoch.as_secs() as i64)
        .bind(library_path)
        .execute(conn)
        .await
    {
        Ok(_) => {
            debug!(
                "last_successfull_scan_date updated in database for library id {library_path} : {}",
                since_the_epoch.as_secs()
            )
        }
        Err(e) => debug!("last_successfull_scan_date update failed : {e}"),
    };
}

/// when a new directory is found or uploaded, insert it
pub async fn insert_new_dir(directory: &DirectoryInfo, ulid: Option<&str>, conn: &Pool<Sqlite>) {
    // generate ulid if needed
    let ulid = match ulid {
        Some(ulid) => ulid.to_string(),
        None => Ulid::new().to_string(),
    };
    // insert_query
    match sqlx::query(
        "INSERT OR REPLACE INTO directories(id, name, parent_path)
                    VALUES(?, ?, ?);",
    )
    .bind(ulid)
    .bind(&directory.name)
    .bind(&directory.parent_path)
    .execute(conn)
    .await
    {
        Ok(_) => {
            debug!("directory update successfull")
        }
        Err(e) => error!("directory infos insert failed : {e}"),
    };
}
