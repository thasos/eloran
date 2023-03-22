use crate::sqlite;

use compress_tools::*;
use epub::doc::EpubDoc;
use image::imageops::FilterType;
use image::DynamicImage;
use jwalk::WalkDirGeneric;
use pdf::object::*;
use sqlx::Sqlite;
use sqlx::{pool::Pool, Row};
use std::fs::File;
use std::io::Cursor;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::runtime::Runtime;
use ulid::Ulid;

/// File struct, match database fields
/// id|name|parent_path|read_status|scan_me|added_date|format|size|total_pages|current_page
#[derive(Debug, Default, Clone, sqlx::FromRow, PartialEq)]
pub struct FileInfo {
    pub id: String,
    pub name: String,
    pub parent_path: String,
    // no bool in sqlite :( , `stored as integers 0 (false) and 1 (true)`
    // see https://www.sqlite.org/datatype3.html
    pub read_status: i8,
    pub scan_me: i8,
    pub added_date: i64,
    pub format: String,
    // pub format: Format,
    // TODO make an Option<i64> if we want to print "unknow" in UI
    // i64 because no u64 with sqlite...
    pub size: i64,
    pub total_pages: i32,
    pub current_page: i32,
}
impl FileInfo {
    pub fn new() -> FileInfo {
        FileInfo {
            // TODO default id ? 🤮
            id: "666".to_string(),
            name: "".to_string(),
            parent_path: "".to_string(),
            added_date: 0,
            read_status: 0,
            scan_me: 1,
            format: "".to_string(),
            // format: Format::Other,
            size: 0,
            total_pages: 0,
            current_page: 0,
        }
    }
}

// sqlx::FromRow not compatible with enums, need an alternative
// #[derive(Debug, Default, Clone, PartialEq)]
// /// Format supported
// pub enum Format {
//     Epub,
//     Pdf,
//     Cbr,
//     Cbz,
//     Txt,
//     #[default]
//     Other,
// }
// impl Format {
//     pub fn as_str(&self) -> &str {
//         match &self {
//             Format::Epub => "epub",
//             Format::Pdf => "pdf",
//             Format::Cbr => "cbr",
//             Format::Cbz => "cbz",
//             Format::Txt => "txt",
//             Format::Other => "Not supported",
//             _ => "other",
//         }
//     }
// }

/// Directory struct, match database fields
/// id|name|parent_path
#[derive(Debug, Default, Clone, sqlx::FromRow, PartialEq)]
pub struct DirectoryInfo {
    pub id: String,
    pub name: String,
    pub parent_path: String,
}

/// try to extract a maximum of informations from the file and set default fields
fn extract_file_infos(entry: &Path) -> FileInfo {
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
    let format: Vec<&str> = filename.rsplit('.').collect();
    let format = format[0].to_string();
    // TODO enum for file type (and "not supported" if fot in members)
    // let format = match format[0] {
    //     "epub" => Format::Epub,
    //     _ => Format::Other,
    // };

    // construct
    FileInfo {
        // TODO default id ? 🤮
        id: "666".to_string(),
        name: filename,
        parent_path,
        added_date: since_the_epoch.as_secs() as i64,
        read_status: 0,
        scan_me: 1,
        format,
        size: size.unwrap_or(0) as i64,
        total_pages: 0,
        current_page: 0,
    }
}

/// when a new file is found or uploaded, insert all values found
/// return file id
async fn insert_new_file(file: &mut FileInfo, ulid: Option<&str>, conn: &Pool<Sqlite>) {
    // generate ulid if needed
    let ulid = match ulid {
        Some(ulid) => ulid.to_string(),
        None => Ulid::new().to_string(),
    };
    file.id = ulid;
    match sqlx::query(
        "INSERT OR REPLACE INTO files(id, name, parent_path, size, added_date, scan_me, read_status, format, current_page, total_pages)
                    VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?);")
        .bind(&file.id)
        // escape ' with '' in sqlite...
        .bind(file.name.replace('\'', "''"))
        .bind(file.parent_path.replace('\'', "''"))
        .bind(file.size)
        .bind(file.added_date)
        .bind(file.scan_me)
        .bind(file.read_status)
        .bind(&file.format)
        .bind(file.current_page)
        .bind(file.total_pages)
        .execute(conn).await {
        Ok(_) => {
            debug!("file insertion successfull")
        }
        Err(e) => error!("file insertion failed : {e}"),
    };
    extract_page_number(file, conn).await;
    extract_cover(file, conn).await;
}

/// delete a file in database
async fn delete_file(file: &FileInfo, conn: &Pool<Sqlite>) {
    match sqlx::query("DELETE FROM files WHERE name = ? AND parent_path = ?;")
        .bind(file.name.replace('\'', "''"))
        .bind(file.parent_path.replace('\'', "''"))
        .execute(conn)
        .await
    {
        Ok(_) => {
            info!("file {}/{} deleted", file.name, file.parent_path)
        }
        Err(e) => error!("delete ko : {}", e),
    }
}

/// get all file in a directory path from database
async fn get_files_from_directory(
    parent_path: &str,
    directory_name: &str,
    conn: &Pool<Sqlite>,
) -> Vec<FileInfo> {
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

/// get all diretories in a path from database
async fn get_registered_directories(conn: &Pool<Sqlite>) -> Vec<DirectoryInfo> {
    let registered_directories: Vec<DirectoryInfo> =
        match sqlx::query_as("SELECT * FROM directories ;")
            .fetch_all(conn)
            .await
        {
            Ok(file_found) => file_found,
            Err(e) => {
                error!("unable to retrieve file infos from database : {}", e);
                let empty_list: Vec<DirectoryInfo> = Vec::new();
                empty_list
            }
        };
    registered_directories
}

/// delete a directory in database
async fn delete_directory(directory: &DirectoryInfo, conn: &Pool<Sqlite>) {
    match sqlx::query(&format!(
        "DELETE FROM directories WHERE name = '{}' AND parent_path = '{}';
         DELETE FROM files WHERE parent_path = '{}/{}'",
        directory.name.replace('\'', "''"),
        directory.parent_path.replace('\'', "''"),
        directory.parent_path.replace('\'', "''"),
        directory.name.replace('\'', "''"),
    ))
    .execute(conn)
    .await
    {
        Ok(_) => {
            info!(
                "directory {}/{} deleted",
                directory.name, directory.parent_path
            )
        }
        Err(e) => error!("delete ko : {}", e),
    }
}

/// return a directory if it exists in database
async fn check_if_directory_exists(
    parent_path: &str,
    directory_name: &str,
    conn: &Pool<Sqlite>,
) -> Vec<DirectoryInfo> {
    let directory_found: Vec<DirectoryInfo> =
        match sqlx::query_as("SELECT * FROM directories WHERE name = ? AND parent_path = ?;")
            .bind(directory_name.replace('\'', "''"))
            .bind(parent_path.replace('\'', "''"))
            .fetch_all(conn)
            .await
        {
            Ok(dir_found) => dir_found,
            Err(e) => {
                error!("unable to retrieve file infos from database : {}", e);
                let empty_list: Vec<DirectoryInfo> = Vec::new();
                empty_list
            }
        };
    directory_found
}

/// return a file if it exists in database
async fn check_if_file_exists(
    parent_path: &str,
    filename: &str,
    conn: &Pool<Sqlite>,
) -> Vec<FileInfo> {
    let file_found: Vec<FileInfo> =
        match sqlx::query_as("SELECT * FROM files WHERE name = ? AND parent_path = ?;")
            .bind(filename.replace('\'', "''"))
            .bind(parent_path.replace('\'', "''"))
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
    file_found
}

/// get last successfull scan date in EPOCH format from database
async fn get_last_successfull_scan_date(conn: &Pool<Sqlite>) -> Duration {
    let last_successfull_scan_date: i64 = match sqlx::query(
        "SELECT last_successfull_scan_date FROM core WHERE id = 1",
    )
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

/// update last successfull scan date in EPOCH format in database
async fn update_last_successfull_scan_date(conn: &Pool<Sqlite>) {
    // le at_least_one_insert_or_delete est pas bon car si rien change, c'est ok
    let now = SystemTime::now();
    let since_the_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
    match sqlx::query("UPDATE core SET last_successfull_scan_date = ? WHERE id = 1;")
        .bind(since_the_epoch.as_secs() as i64)
        .execute(conn)
        .await
    {
        Ok(_) => debug!("last_successfull_scan_date updated in database"),
        Err(e) => debug!("last_successfull_scan_date update failed : {e}"),
    };
}

/// when a new directory is found or uploaded, insert it
async fn insert_new_dir(directory: &DirectoryInfo, ulid: Option<&str>, conn: &Pool<Sqlite>) {
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
    // escape ' with '' in sqlite...
    .bind(directory.name.replace('\'', "''"))
    .bind(directory.parent_path.replace('\'', "''"))
    .execute(conn)
    .await
    {
        Ok(_) => {
            debug!("directory update successfull")
        }
        Err(e) => error!("directory infos insert failed : {e}"),
    };
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
/// insert them in the process_read_dir fn of jwalk crate
fn walk_recent_files_and_insert(
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
                        // insert here to benefit the jwalk parallelism
                        // file_infos need to be mutable for ulid genereation
                        let mut file_infos = extract_file_infos(dir_entry.path().as_path());
                        let filename = file_infos.name.clone();
                        let parent_path = file_infos.parent_path.clone();
                        // create a new tokio runtime for inserts
                        // we need it to use async fns create_sqlite_pool and insert_new_file
                        let rt = Runtime::new()
                            .expect("runtime creation for insertions while scanning library");
                        rt.block_on(async move {
                            let conn = sqlite::create_sqlite_pool().await;
                            let file_found = check_if_file_exists(
                                parent_path.as_str(),
                                filename.as_str(),
                                &conn,
                            )
                            .await;
                            if file_found.is_empty() {
                                // new file
                                info!("new file found : {}/{}", parent_path, filename);
                                insert_new_file(&mut file_infos, None, &conn).await;
                            } else if file_found.len() == 1 {
                                // 1 file found, ok update it
                                info!("file modified : {}/{}", parent_path, filename);
                                let ulid_found = &file_found[0].id;
                                insert_new_file(&mut file_infos, Some(ulid_found), &conn).await;
                            } else {
                                // multiple id for a file ? wrong !!
                                // TODO propose repair or full rescan
                                error!(
                                    "base possibly corrupted, multiple id found for file {}/{}",
                                    parent_path, filename
                                );
                            }
                        });
                        // flag file for insert
                        dir_entry.client_state = true;
                    }
                }
            });
        })
}

/// scan library path and add files in db
// batch insert -> no speed improvement
// TODO check total number file found, vs total in db (for insert errors) ?
pub async fn scan_routine(library_path: &Path, sleep_time: Duration) {
    // register library_path in database if not present
    let conn = sqlite::create_sqlite_pool().await;

    // main loop
    loop {
        sqlite::set_library_path(library_path, &conn).await;
        if !library_path.is_dir() {
            error!("{} does not exists", library_path.to_string_lossy());
        } else {
            debug!(
                "path \"{}\" found and is a directory",
                library_path.to_string_lossy()
            );

            // retrieve last_successfull_scan_date, 0 if first time
            let last_successfull_scan_date = get_last_successfull_scan_date(&conn).await;

            // recent directories to find new and removed files
            let updated_dir_list = walk_recent_dir(library_path, last_successfull_scan_date);

            // loop on modified dirs
            for entry in updated_dir_list.into_iter().flatten() {
                if entry.client_state {
                    let current_directory = DirectoryInfo {
                        id: "666".to_string(),
                        name: entry.file_name.to_string_lossy().to_string(),
                        parent_path: entry.parent_path.to_string_lossy().to_string(),
                    };
                    info!(
                        "new changes in dir {}/{}, need to scan it",
                        current_directory.name, current_directory.parent_path,
                    );
                    // TODO add dir in db only in fot exists
                    // TODO use struct ....
                    let directory_found = check_if_directory_exists(
                        &current_directory.parent_path,
                        &current_directory.name,
                        &conn,
                    )
                    .await;
                    // new directory
                    if directory_found.is_empty() && !current_directory.parent_path.is_empty() {
                        insert_new_dir(&current_directory, None, &conn).await;
                    }

                    // search for removed files
                    // retrieve file list in database for current directory
                    let registered_files = get_files_from_directory(
                        &entry.parent_path.to_string_lossy(),
                        &entry.file_name.to_string_lossy(),
                        &conn,
                    )
                    .await;
                    // check if files exists for current directory, delete in database if not
                    for file in registered_files {
                        let full_path = format!("{}/{}", file.parent_path, file.name);
                        let file_path = Path::new(&full_path);
                        if !file_path.is_file() {
                            delete_file(&file, &conn).await;
                        }
                    }
                }
            }

            // removed directory
            let registered_directories = get_registered_directories(&conn).await;
            // check if files exists for current directory, delete in database if not
            for directory in registered_directories {
                let full_path = format!("{}/{}", directory.parent_path, directory.name);
                let directory_path = Path::new(&full_path);
                if !directory_path.is_dir() {
                    info!(
                        "directory {} not found but still present in database, deleting",
                        full_path
                    );
                    delete_directory(&directory, &conn).await;
                }
            }

            // recent files : added and modified files
            let recent_file_list =
                walk_recent_files_and_insert(library_path, last_successfull_scan_date);
            for entry in recent_file_list.into_iter().flatten() {
                if entry.client_state {
                    debug!(
                        "insert file {}/{}",
                        entry.parent_path.to_string_lossy(),
                        entry.file_name.to_string_lossy()
                    );
                }
            }
            // end scanner, update date if successfull
            // TODO comment check si successfull ?
            // le at_least_one_insert_or_delete est pas bon car si rien change, c'est ok
            update_last_successfull_scan_date(&conn).await;
        }
        // TODO true schedule, last scan status in db...
        debug!(
            "stop scanning, sleeping for {} seconds",
            sleep_time.as_secs()
        );
        tokio::time::sleep(sleep_time).await;
    }
}

pub fn dynamic_image_to_vec_u8(image: DynamicImage) -> Vec<u8> {
    let mut buf = Cursor::new(vec![]);
    image.write_to(&mut buf, image::ImageFormat::Jpeg).unwrap();
    let vec_u8_image = buf.get_ref();
    vec_u8_image.to_owned()
}

pub async fn extract_cover(file: &FileInfo, conn: &Pool<Sqlite>) {
    let dynamic_image_cover = match file.format.as_str() {
        "epub" => extract_epub_cover(file),
        "pdf" => extract_pdf_cover(file),
        "cbz" | "cbr" | "cb7" => extract_comic_cover(file),
        _ => None,
    };

    if let Some(cover) = dynamic_image_cover {
        let buffered_u8_cover = dynamic_image_to_vec_u8(cover);
        sqlite::insert_cover(file, &buffered_u8_cover, conn).await
    }
}

pub async fn extract_page_number(file: &FileInfo, conn: &Pool<Sqlite>) {
    match file.format.as_str() {
        "epub" => extract_epub_page_number(file, conn).await,
        "pdf" => extract_pdf_page_number(file, conn).await,
        "cbz" | "cbr" | "cb7" => extract_comic_page_number(file, conn).await,
        _ => (),
    }
}

pub async fn extract_pdf_page_number(file: &FileInfo, conn: &Pool<Sqlite>) {
    let full_path = format!("{}/{}", file.parent_path, file.name);
    let mut total_pages = 0;

    // no await in this scope, cause error trait `Handler<_, _, _>` is not implemented
    // use macro #[axum::debug_handler] on handler to see details
    if let Ok(doc) = pdf::file::File::open(&full_path) {
        total_pages = doc.num_pages();
    };

    sqlite::insert_total_pages(file, total_pages as i32, conn).await;
}

// TODO error/warn message for each `None` in arms
// ⚠️  "the image crate is known to be quite slow when compiled in debug mode"
// from https://www.reddit.com/r/rust/comments/k1wjix/why_opening_of_images_is_so_slow/
pub fn extract_pdf_cover(file: &FileInfo) -> Option<image::DynamicImage> {
    let full_path = format!("{}/{}", file.parent_path, file.name);
    let mut cover: Option<image::DynamicImage> = None;

    if let Ok(doc) = pdf::file::File::open(&full_path) {
        if let Ok(page) = doc.get_page(0) {
            let resources = page.resources().unwrap();

            let mut cover_images: Vec<RcRef<XObject>> = vec![];
            cover_images.extend(
                resources
                    .xobjects
                    .iter()
                    .map(|(_name, &ressource)| doc.get(ressource).unwrap())
                    .filter(|o| matches!(**o, XObject::Image(_))),
            );

            let first_object = cover_images.first()?;
            let image = match **first_object {
                XObject::Image(ref im) => Some(im),
                _ => None,
            };
            let data = match image?.raw_image_data(&doc) {
                Ok(toot) => Some(toot.0),
                Err(e) => {
                    error!("unable to read raw image for file {} : {e}", &file.name);
                    None
                }
            };
            let vec_cover = data?.to_vec();

            // resize and insert
            match image::load_from_memory(&vec_cover) {
                Ok(img) => cover = Some(resize_cover(img)),
                Err(_) => {
                    warn!("I can't decode cover image for file {full_path}");
                }
            };
            cover
        } else {
            None
        }
    } else {
        error!("unable to load pdf file {}", &full_path);
        None
    }
}

pub async fn extract_epub_page_number(file: &FileInfo, conn: &Pool<Sqlite>) {
    let full_path = format!("{}/{}", file.parent_path, file.name);
    if let Ok(doc) = EpubDoc::new(&full_path) {
        let total_pages = doc.get_num_pages();
        sqlite::insert_total_pages(file, total_pages as i32, conn).await;
    };
}

// TODO error/warn message for each `None` in arms
pub fn extract_epub_cover(file: &FileInfo) -> Option<image::DynamicImage> {
    let full_path = format!("{}/{}", file.parent_path, file.name);
    if let Ok(mut doc) = EpubDoc::new(&full_path) {
        // extract cover
        let vec_cover = if let Some(cover) = doc.get_cover() {
            // (Vec<u8>, String) : img and mime-type
            // we only need the first tuple element
            cover.0
        } else {
            // (vec![], String::new())
            vec![]
        };
        match image::load_from_memory(&vec_cover) {
            // match image::load_from_memory(&cover.0) {
            Ok(img) => {
                let cover = resize_cover(img);
                Some(cover)
                // sqlite::insert_cover(file, cover, conn).await;
            }
            Err(_) => {
                warn!("I can't decode cover image for file {full_path}");
                None
            }
        }
    } else {
        None
    }
}

// filter a list of files in archives to keep only images with thier indexes
// fn extract_comic_image_list(archive: &str) -> Vec<(usize, String)> {
pub fn extract_comic_image_list(archive: &File) -> Vec<String> {
    // thread '<unnamed>' panicked at 'list_archive_files: Utf(Utf8Error { valid_up_to: 20, error_len: Some(1) })', src/scanner.rs:716:55

    let comic_file_list = match list_archive_files(archive) {
        Ok(list) => list,
        Err(e) => {
            error!("unable to extract file list form archive : {e}");
            Vec::default()
        }
    };
    // TODO use drain_filter when it will be stable
    // see https://github.com/rust-lang/rust/issues/43244
    let mut image_list = Vec::default();
    for file_path in comic_file_list.into_iter() {
        if file_path.to_lowercase().contains(".jpg")
            || file_path.to_lowercase().contains(".jpeg")
            || file_path.to_lowercase().contains(".png")
        {
            image_list.push(file_path);
        }
    }
    // sometime the archive does not begin by image 01...
    image_list.sort();
    image_list
}

pub async fn extract_comic_page_number(file: &FileInfo, conn: &Pool<Sqlite>) {
    let compressed_comic_file =
        File::open(format!("{}/{}", file.parent_path, file.name)).expect("file open");
    let file_list = extract_comic_image_list(&compressed_comic_file);
    let total_pages = file_list.len();
    sqlite::insert_total_pages(file, total_pages as i32, conn).await;
}

pub fn extract_comic_cover(file: &FileInfo) -> Option<image::DynamicImage> {
    let archive_path = &format!("{}/{}", file.parent_path, file.name);
    let compressed_comic_file = File::open(archive_path).expect("file open");
    // get images list from archive
    let comic_file_list = extract_comic_image_list(&compressed_comic_file);
    // set path file wanted from page index
    let image_path_in_achive = match comic_file_list.first() {
        Some(path) => path,
        None => {
            error!("could not retrive cover in archive");
            ""
        }
    };
    // uncompress corresponding image
    let mut vec_cover: Vec<u8> = Vec::default();
    // RAR need to reopen file... why ? and why rar only ?
    let compressed_comic_file = File::open(archive_path).expect("file open");
    match uncompress_archive_file(&compressed_comic_file, &mut vec_cover, image_path_in_achive) {
        Ok(_) => (),
        Err(e) => error!(
            "unable to extract path '{}' from file '{}' : {e}",
            image_path_in_achive, file.name
        ),
    }

    if let Ok(cover) = image::load_from_memory(&vec_cover) {
        Some(cover)
    } else {
        None
    }
}
// TODO easy testing here...
pub fn resize_cover(cover: image::DynamicImage) -> image::DynamicImage {
    // see doc https://docs.rs/image/0.24.5/image/imageops/enum.FilterType.html
    // for quality of resize (Nearest is ugly)
    // TODO do not keep ratio ? crop ? the max heigh is the most important
    cover.resize_to_fill(150, 230, FilterType::Triangle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite;
    use sqlx::{migrate::MigrateDatabase, Sqlite};
    use std::fs::{self, File};
    use std::io::prelude::*;
    use std::path::Path;

    fn create_fake_library(library_path: &Path) -> std::io::Result<()> {
        fs::create_dir(library_path)?;
        fs::create_dir(library_path.join("Asterix"))?;
        let mut file = File::create(library_path.join("Asterix/T01 - Asterix le Gaulois.pdf"))?;
        file.write(b"lalalalala")?;
        file.flush()?;
        let mut file = File::create(library_path.join("Asterix/T02 - La Serpe d'Or.pdf"))?;
        file.write(b"lalalalala")?;
        file.flush()?;
        fs::create_dir(library_path.join("Goblin's"))?;
        File::create(library_path.join("Goblin's/T01.cbz"))?;
        File::create(library_path.join("Goblin's/T02.cbz"))?;
        fs::create_dir(library_path.join("H.P. Lovecraft"))?;
        fs::create_dir(library_path.join("H.P. Lovecraft/Le Cauchemar d'Innsmouth (310)"))?;
        File::create(
            library_path.join("H.P. Lovecraft/Le Cauchemar d'Innsmouth (310)/metadata.opf"),
        )?;
        File::create(library_path.join("H.P. Lovecraft/Le Cauchemar d'Innsmouth (310)/cover.jpg"))?;
        File::create(library_path.join("H.P. Lovecraft/Le Cauchemar d'Innsmouth (310)/Le Cauchemar d'Innsmouth - Howard Phillips Lovecraft.epub"))?;
        fs::create_dir(library_path.join("Dragonlance"))?;
        Ok(())
    }

    fn delete_fake_library(library_path: &Path) -> std::io::Result<()> {
        fs::remove_dir_all(library_path)?;
        Ok(())
    }

    #[test]
    fn test_extract_new_file() {
        // create library
        let library_path = Path::new("library_new_file");
        create_fake_library(library_path).unwrap_or(());
        // run test
        let validation_file =
            extract_file_infos(&library_path.join("Asterix/T01 - Asterix le Gaulois.pdf"));
        let skeletion_file = FileInfo {
            name: "T01 - Asterix le Gaulois.pdf".to_string(),
            parent_path: format!("{}/Asterix", library_path.to_string_lossy()),
            read_status: 0,
            scan_me: 1,
            format: "pdf".to_string(),
            size: 10,
            total_pages: 0,
            current_page: 0,
            // id and added_date are random, so we take them from validation_file
            added_date: validation_file.added_date.clone(),
            id: validation_file.id.clone(),
        };
        assert_eq!(validation_file, skeletion_file);
        // delete library
        delete_fake_library(library_path).unwrap_or(());
    }

    #[tokio::test]
    async fn test_insert_new_file() {
        // init database
        sqlite::init_database().await;
        // run test
        let mut skeletion_file = FileInfo {
            name: "T01 - Asterix le Gaulois.pdf".to_string(),
            parent_path: "library/Asterix".to_string(),
            read_status: 0,
            scan_me: 1,
            format: "pdf".to_string(),
            size: 10,
            total_pages: 0,
            current_page: 0,
            // id and added_date are random, so we take them from validation_file
            added_date: 666,
            id: "666".to_string(),
        };
        let conn = sqlite::create_sqlite_pool().await;
        insert_new_file(&mut skeletion_file, None, &conn).await;
        let file_from_base: Vec<FileInfo> =
            match sqlx::query_as("SELECT * FROM files WHERE parent_path = ?;")
                .bind(&skeletion_file.parent_path.replace('\'', "''"))
                .fetch_all(&conn)
                .await
            {
                Ok(file_found) => file_found,
                Err(e) => {
                    error!("unable to retrieve file infos from database : {}", e);
                    let empty_list: Vec<FileInfo> = Vec::new();
                    empty_list
                }
            };
        assert_eq!(file_from_base.first().unwrap().name, skeletion_file.name);
        // delete database
        Sqlite::drop_database(crate::DB_URL);
    }

    #[test]
    fn test_walkdir() {
        // create library
        let library_path = Path::new("library_walkdir");
        create_fake_library(library_path).unwrap_or(());
        // run test
        let timestamp_flag = Duration::from_secs(666);
        // recent directories
        let dir_list = walk_recent_dir(library_path, timestamp_flag);
        let mut dir_list_path: Vec<String> = vec![];
        for entry in dir_list.into_iter().flatten() {
            if entry.client_state {
                dir_list_path.push(entry.path().to_string_lossy().to_string());
            }
        }
        let mut check_dir_list_path: Vec<String> = vec![
            "library_walkdir".to_string(),
            "library_walkdir/Asterix".to_string(),
            "library_walkdir/Goblin's".to_string(),
            "library_walkdir/H.P. Lovecraft".to_string(),
            "library_walkdir/H.P. Lovecraft/Le Cauchemar d'Innsmouth (310)".to_string(),
            "library_walkdir/Dragonlance".to_string(),
        ];
        dir_list_path.sort();
        check_dir_list_path.sort();
        assert_eq!(dir_list_path, check_dir_list_path);
        // recent files
        let file_list = walk_recent_files_and_insert(library_path, timestamp_flag);
        let mut file_list_path: Vec<String> = vec![];
        for entry in file_list.into_iter().flatten() {
            if entry.client_state {
                file_list_path.push(entry.path().to_string_lossy().to_string());
            }
        }
        let mut check_file_list_path: Vec<String> = vec![
            "library_walkdir/Asterix/T01 - Asterix le Gaulois.pdf".to_string(),
            "library_walkdir/Asterix/T02 - La Serpe d'Or.pdf".to_string(),
            "library_walkdir/Goblin's/T01.cbz".to_string(),
            "library_walkdir/Goblin's/T02.cbz".to_string(),
            "library_walkdir/H.P. Lovecraft/Le Cauchemar d'Innsmouth (310)/cover.jpg".to_string(),
            "library_walkdir/H.P. Lovecraft/Le Cauchemar d'Innsmouth (310)/metadata.opf".to_string(),
            "library_walkdir/H.P. Lovecraft/Le Cauchemar d'Innsmouth (310)/Le Cauchemar d'Innsmouth - Howard Phillips Lovecraft.epub".to_string(),
        ];
        file_list_path.sort();
        check_file_list_path.sort();
        assert_eq!(file_list_path, check_file_list_path);
        // delete database
        delete_fake_library(library_path).unwrap_or(());
    }

    // #[test]
    // fn test_delete_file() {
    //     // TODO
    //     todo!();
    // }
}
