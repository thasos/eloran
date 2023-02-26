use crate::sqlite;
use jwalk::WalkDirGeneric;
use sqlx::Row;
// use sqlx::pool::PoolConnection;
// use sqlx::Sqlite;
use std::path::Path;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use ulid::Ulid;

/// id|filename|parent_path|read_status|scan_me|added_date|file_type|size|total_pages|current_page
// TODO rename to Publication ? (match comic and book)
#[derive(Debug, Default, Clone, sqlx::FromRow)]
pub struct FileInfo {
    pub id: String,
    pub filename: String,
    pub parent_path: String,
    // no bool in sqlite :( , `stored as integers 0 (false) and 1 (true)`
    // see https://www.sqlite.org/datatype3.html
    pub read_status: i8,
    pub scan_me: i8,
    pub added_date: i64,
    pub file_type: String,
    // TODO make an Option<i64> if we want to print "unknow" in UI
    // i64 because no u64 with sqlite...
    pub size: i64,
    pub total_pages: i32,
    pub current_page: i32,
}

/// try to extract a maximum of informations from the file and set default fields
fn extract_new_file_infos(entry: &Path) -> FileInfo {
    // filename
    let filename = match entry.file_name() {
        Some(name) => name.to_str().unwrap(),
        None => "unknow file name",
    }
    .to_string();
    // parent path
    let parent_path = match entry.parent() {
        Some(path) => path.to_str().unwrap(),
        None => "unknow path",
    }
    .to_string();
    // current date in unixepoch format
    let now = SystemTime::now();
    let since_the_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
    // file size
    let size = match entry.metadata() {
        Ok(size) => Some(size.len()),
        Err(e) => {
            warn!("unable to determine size for file {} : {}", filename, e);
            None
        }
    };
    // file type
    // TODO enum for file type (and "not supported" if fot in members)
    let file_type: Vec<&str> = filename.rsplit('.').collect();
    let file_type = file_type[0].to_string();

    // construct
    FileInfo {
        // TODO default id ? 🤮
        id: "666".to_string(),
        filename,
        parent_path,
        added_date: since_the_epoch.as_secs() as i64,
        // defaults bools for new file
        // 0 = false, 1 = true
        read_status: 0,
        scan_me: 1,
        file_type,
        size: size.unwrap_or(0) as i64,
        total_pages: 0,
        current_page: 0,
    }
}

/// when a new file is found or uploaded, insert all values found
fn insert_new_file(file: &FileInfo, ulid: Option<&str>) -> String {
    // generate ulid
    let ulid = match ulid {
        Some(ulid) => ulid.to_string(),
        None => Ulid::new().to_string(),
    };
    // prepare query
    let insert_query = format!(
        // "INSERT INTO library(id, filename, parent_path, size, added_date, scan_me, read_status, file_type, current_page, total_pages)
        "INSERT OR REPLACE INTO library(id, filename, parent_path, size, added_date, scan_me, read_status, file_type, current_page, total_pages)
                    VALUES('{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}');",
        ulid,
        // escape ' with '' in sqlite...
        file.filename.replace('\'', "''"),
        file.parent_path.replace('\'', "''"),
        file.size,
        file.added_date,
        file.scan_me,
        file.read_status,
        file.file_type,
        file.current_page,
        file.total_pages);
    insert_query
}

/// walk library dir and return list of files modified after the last successfull scan
/// directory updated match new file, removed file
fn walk_recent_dir(
    library_path: &Path,
    last_successfull_scan_date: Duration,
) -> WalkDirGeneric<(usize, bool)> {
    debug!("start walkdir for recent directories");
    WalkDirGeneric::<(usize, bool)>::new(library_path)
        .skip_hidden(true)
        .process_read_dir(move |_depth, _path, _read_dir_state, children| {
            children.iter_mut().for_each(|dir_entry_result| {
                if let Ok(dir_entry) = dir_entry_result {
                    // retrieve metadatas for mtime
                    // TODO too much unwraps
                    let dir_entry_metadata = dir_entry.metadata().unwrap();
                    let dir_entry_modified_date = dir_entry_metadata.modified().unwrap()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap();
                    // filter on updated dirs from last scan
                    if dir_entry.file_type().is_dir() && dir_entry_modified_date > last_successfull_scan_date
                    {
                        debug!(
                            "modified time {}, greater than last successfull scan {} for directory {}",
                            dir_entry_modified_date.as_secs(),
                            last_successfull_scan_date.as_secs(),
                            dir_entry.file_name().to_string_lossy()
                        );
                        // flag dir for scan
                        dir_entry.client_state = true;
                    }
                }
            });
        })
}

/// walk library dir and return list of files modified after the last successfull scan
fn walk_recent_files(
    library_path: &Path,
    last_successfull_scan_date: Duration,
) -> WalkDirGeneric<(usize, bool)> {
    // recursive walk_dir
    WalkDirGeneric::<(usize, bool)>::new(library_path)
        .skip_hidden(true)
        .process_read_dir(move |_depth, _path, _read_dir_state, children| {
            children.iter_mut().for_each(|dir_entry_result| {
                if let Ok(dir_entry) = dir_entry_result {
                    // retrieve metadatas for mtime
                    // TODO too much unwraps
                    let dir_entry_metadata = dir_entry.metadata().unwrap();
                    let dir_entry_modified_date = dir_entry_metadata
                        .modified()
                        .unwrap()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap();
                    // check mtime for files only, because directories will be not crossed
                    // without this check
                    if dir_entry.file_type().is_file()
                        && dir_entry_modified_date > last_successfull_scan_date
                    {
                        debug!(
                            "modified time {}, greater than last successfull scan {} for file {}",
                            dir_entry_modified_date.as_secs(),
                            last_successfull_scan_date.as_secs(),
                            dir_entry.file_name().to_string_lossy()
                        );
                        // flag file for insert
                        dir_entry.client_state = true;
                    }
                }
            });
        })
}

/// scan library path and add files in db
// batch insert ? -> no speed improvement
// TODO check total number file found, vs total in db (for insert errors) ?
pub async fn scan_routine() {
    // TODO lib path in conf, need more checks of library ?
    // let library_path = "library";
    let library_path = "/home/thasos/books";
    info!("start scanner routine on library {}", library_path);

    loop {
        let library_path = Path::new(library_path);
        if library_path.is_dir() {
            debug!(
                "path \"{}\" found and is a directory",
                library_path.to_string_lossy()
            );

            // create pool connexion
            let mut conn = sqlite::create_sqlite_connection().await;

            // retrieve last_successfull_scan_date, 0 if first time
            let last_successfull_scan_date: i64 = match sqlx::query(
                "SELECT last_successfull_scan_date FROM core WHERE id = 1",
            )
            .fetch_one(&mut conn)
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
            let last_successfull_scan_date: Duration =
                Duration::from_secs(u64::try_from(last_successfull_scan_date).unwrap());

            // recursive walk_dir

            // recent directories : find new and removed files
            let updated_dir_list = walk_recent_dir(library_path, last_successfull_scan_date);

            // loop on modified dirs
            for entry in updated_dir_list.into_iter().flatten() {
                if entry.client_state {
                    info!(
                        "new changes in dir {}/{}, need to scan it",
                        entry.parent_path.to_string_lossy(),
                        entry.file_name.to_string_lossy()
                    );

                    // search for removed files
                    // retrieve file list in database for current directory
                    let registered_files: Vec<FileInfo> = match sqlx::query_as(&format!(
                        "SELECT * FROM library WHERE parent_path = '{}/{}'",
                        entry.parent_path.to_string_lossy().replace('\'', "''"),
                        entry.file_name.to_string_lossy().replace('\'', "''")
                    ))
                    .fetch_all(&mut conn)
                    .await
                    {
                        Ok(file_found) => file_found,
                        Err(e) => {
                            error!("unable to retrieve file infos from database : {}", e);
                            let empty_list: Vec<FileInfo> = Vec::new();
                            empty_list
                        }
                    };
                    // check if files exists for current directory
                    for file in registered_files {
                        let full_path = format!("{}/{}", file.parent_path, file.filename);
                        let file_path = Path::new(&full_path);
                        if !file_path.is_file() {
                            match sqlx::query(&format!(
                                "DELETE FROM library WHERE filename = '{}' AND parent_path = '{}';",
                                file.filename.replace('\'', "''"),
                                file.parent_path.replace('\'', "''")
                            ))
                            .execute(&mut conn)
                            .await
                            {
                                Ok(_) => {
                                    info!("file {}/{} deleted", file.filename, file.parent_path)
                                }
                                Err(e) => error!("delete ko : {}", e),
                            }
                        }
                    }
                }
            }

            // recent files : added and modified files
            let recent_file_list = walk_recent_files(library_path, last_successfull_scan_date);

            // loop on recent files list
            for entry in recent_file_list.into_iter().flatten() {
                if entry.client_state {
                    let file_infos = extract_new_file_infos(entry.path().as_path());
                    let filename = &file_infos.filename;
                    let parent_path = &file_infos.parent_path;
                    let file_found: Vec<FileInfo> = match sqlx::query_as(&format!(
                        "SELECT * FROM library WHERE filename = '{}' AND parent_path = '{}'",
                        filename.replace('\'', "''"),
                        parent_path.replace('\'', "''")
                    ))
                    .fetch_all(&mut conn)
                    .await
                    {
                        Ok(file_found) => file_found,
                        Err(e) => {
                            error!("unable to retrieve file infos from database : {}", e);
                            let empty_list: Vec<FileInfo> = Vec::new();
                            empty_list
                        }
                    };
                    // new file
                    if file_found.is_empty() {
                        warn!("new file found : {}/{}", parent_path, filename);
                        let insert_query = insert_new_file(&file_infos, None);
                        match sqlx::query(&insert_query).execute(&mut conn).await {
                            Ok(_) => {
                                debug!("file update successfull")
                            }
                            Err(e) => error!("file infos insert failed : {e}"),
                        };
                        // 1 file found, ok update it
                    } else if file_found.len() == 1 {
                        warn!("file modified : {}/{}", parent_path, filename);
                        let ulid_found = &file_found[0].id;
                        let insert_query = insert_new_file(&file_infos, Some(ulid_found));
                        match sqlx::query(&insert_query).execute(&mut conn).await {
                            Ok(_) => {
                                debug!("file update successfull")
                            }
                            Err(e) => error!("file infos insert failed : {e}"),
                        };
                        // multiple id for a file ? wrong !!
                    } else {
                        // TODO propose repair or full rescan
                        error!(
                            "base possibly corrupted, multiple id found for file {}/{}",
                            parent_path, filename
                        );
                    }
                }
            }

            // end scanner, update date if successfull
            // TODO comment check si successfull ?
            // le at_least_one_insert_or_delete est pas bon car si rien change, c'est ok
            let now = SystemTime::now();
            let since_the_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
            match sqlx::query(&format!(
                "INSERT OR REPLACE INTO core (id, last_successfull_scan_date)
                    VALUES (1, '{}');",
                since_the_epoch.as_secs() as i64
            ))
            .execute(&mut conn)
            .await
            {
                Ok(_) => debug!("last_successfull_scan_date updated in database"),
                Err(e) => debug!("last_successfull_scan_date update failed : {e}"),
            };

            // TODO true schedule, last scan status in db...
            let sleep_time = Duration::from_secs(5);
            debug!(
                "stop scanning, sleeping for {} seconds",
                sleep_time.as_secs()
            );
            tokio::time::sleep(sleep_time).await;
        }
    }
}

/// extract images and metadatas form files
/// based on file list generated with scan_routine fn
pub async fn _file_extractor_routine() {
    debug!("try to start file extractor routine");
    loop {
        debug!("file extractor loop");
        // TODO true schedule, last scan status in db...
        let sleep_time = Duration::from_secs(3);
        tokio::time::sleep(sleep_time).await;
    }
}
