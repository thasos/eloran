use crate::sqlite;
use jwalk::WalkDirGeneric;
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
async fn insert_new_file(file: &FileInfo) {
    // create pool connexion
    let mut conn = sqlite::create_sqlite_connection().await;
    // generate ulid
    let ulid = Ulid::new().to_string();
    // insert in db
    let insert_status = sqlx::query(&format!(
        "INSERT INTO library(id, filename, parent_path, size, added_date, scan_me, read_status, file_type, current_page, total_pages)
                    VALUES('{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}');",
        ulid,
        file.filename,
        file.parent_path,
        file.size,
        file.added_date,
        file.scan_me,
        file.read_status,
        file.file_type,
        file.current_page,
        file.total_pages,
    ))
    .execute(&mut conn)
    .await;
    match insert_status {
        Ok(_) => debug!("file infos insert successfull"),
        Err(e) => debug!("file infos insert failed : {e}"),
    };
}

/// scan library path and add files in db
pub async fn scan_routine() {
    let library_path = "library";
    // let library_path = "/home/thasos/.cache";
    debug!("try to start scanner routine");
    loop {
        debug!("scanner loop");
        // TODO lib path in conf
        let library_path = Path::new(library_path);
        if library_path.is_dir() {
            debug!(
                "path \"{}\" found and is a directory",
                library_path.to_string_lossy()
            );

            // create pool connexion
            let mut conn = sqlite::create_sqlite_connection().await;

            // recursive walk_dir
            // TODO use process_read_dir to filter files ?
            let walk_dir = WalkDirGeneric::<(usize, bool)>::new(library_path).skip_hidden(true);
            for entry in walk_dir.into_iter().flatten() {
                // only check files
                if entry.file_type().is_file() {
                    // extract file name and path
                    let file_infos = extract_new_file_infos(entry.path().as_path());
                    let filename = &file_infos.filename;
                    debug!("file {} found", filename);
                    // check if already in db and instert if needed
                    // TODO update if needed (modification date ?)
                    let is_new = sqlx::query(&format!(
                        "SELECT id FROM library WHERE filename = '{}'",
                        filename
                    ))
                    .fetch_all(&mut conn)
                    .await
                    .unwrap();
                    if is_new.is_empty() {
                        debug!("file {} not present in db, try to insert", filename);
                        insert_new_file(&file_infos).await;
                    } else {
                        debug!("file {} already present in db, skipping", filename);
                        // TODO debug à virer
                        // insert_new_file(&file_infos).await;
                    }
                }
            }
        }

        // TODO true schedule, last scan status in db...
        let sleep_time = Duration::from_secs(300);
        tokio::time::sleep(sleep_time).await;
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
