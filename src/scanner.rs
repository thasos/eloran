use crate::sqlite;

use cairo::Context;
use compress_tools::*;
use epub::doc::EpubDoc;
use image::DynamicImage;
use image::imageops::FilterType;
use jwalk::WalkDirGeneric;
use poppler::Document;
use serde::Serialize;
use sqlx::Sqlite;
use sqlx::pool::Pool;
use std::cmp::Ordering;
use std::fmt;
use std::fs::{self, File};
use std::io::Cursor;
use std::os::linux::fs::MetadataExt;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::runtime::Runtime;

/// Library Struct
#[derive(Debug, Default, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct Library {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub last_successfull_scan_date: i64,
    pub last_successfull_extract_date: i64,
    pub file_count: i32,
}
impl Library {
    pub fn new() -> Library {
        Library {
            id: 0,
            name: "".to_string(),
            path: "".to_string(),
            last_successfull_scan_date: 0,
            last_successfull_extract_date: 0,
            file_count: 0,
        }
    }
}

/// File struct, match database fields
/// id|name|parent_path|read_status|scan_me|added_date|format|size|total_pages
// #[derive(Debug, Default, Clone, sqlx::FromRow, PartialEq, Eq, PartialOrd, Ord)]
#[derive(Debug, Default, Clone, sqlx::FromRow, PartialEq, Eq, Serialize)]
pub struct FileInfo {
    pub id: String,
    pub name: String,
    // TODO replace library_name by library_id ? (not sure why I choosed name in 1st place...)
    pub library_name: String,
    pub parent_path: String,
    // no bool in sqlite :( , `stored as integers 0 (false) and 1 (true)`
    // see https://www.sqlite.org/datatype3.html
    pub scan_me: i8,
    pub added_date: i64,
    pub format: Format,
    // pub format: Format,
    // TODO make an Option<i64> if we want to print "unknow" in UI
    // i64 because no u64 with sqlite...
    pub size: i64,
    pub total_pages: i32,
    // list of users id separated by comma : `id1,id2,...`
    pub read_by: String,
    pub bookmarked_by: String,
}
impl FileInfo {
    pub fn new() -> FileInfo {
        FileInfo {
            // TODO default id ? 🤮
            id: "666".to_string(),
            name: "".to_string(),
            library_name: "".to_string(),
            parent_path: "".to_string(),
            added_date: 0,
            scan_me: 1,
            format: Format::Other,
            // format: Format::Other,
            size: 0,
            total_pages: 0,
            read_by: "".to_string(),
            bookmarked_by: "".to_string(),
        }
    }
}
impl PartialOrd for FileInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for FileInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

// sqlx::FromRow not compatible with enums, need an alternative
#[derive(Debug, Default, Clone, sqlx::Type, PartialEq, Eq, Serialize)]
#[sqlx(type_name = "format", rename_all = "lowercase")]
/// Supported formats
pub enum Format {
    Epub,
    Pdf,
    Cbr,
    Cbz,
    Txt,
    Jpg,
    #[default]
    Other,
}
impl Format {
    pub fn as_str(&self) -> &str {
        match &self {
            Format::Epub => "epub",
            Format::Pdf => "pdf",
            Format::Cbr => "cbr",
            Format::Cbz => "cbz",
            Format::Txt => "txt",
            Format::Jpg => "jpg",
            Format::Other => "Not supported",
        }
    }
}
impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Format::Epub => write!(f, "epub"),
            Format::Cbr => write!(f, "cbr"),
            Format::Cbz => write!(f, "cbz"),
            Format::Pdf => write!(f, "pdf"),
            Format::Txt => write!(f, "txt"),
            Format::Jpg => write!(f, "jpg"),
            Format::Other => write!(f, "unknow"),
        }
    }
}

/// Directory struct, match database fields
/// id|name|parent_path
#[derive(Debug, Default, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct DirectoryInfo {
    // TODO need library id for easy deleting
    pub id: String,
    pub name: String,
    pub parent_path: String,
    pub file_count: Option<i32>,
}
impl PartialOrd for DirectoryInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for DirectoryInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

/// try to extract a maximum of informations from the file and set default fields
fn extract_file_infos(library_name: &str, entry: &Path) -> FileInfo {
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
            warn!("unable to determine size for file [{}] : {}", filename, e);
            None
        }
    };
    // file type
    let format: Vec<&str> = filename.rsplit('.').collect();
    // let format = format[0];
    // TODO enum for file type (and "not supported" if fot in members)
    let format = match format[0].to_lowercase().as_str() {
        "epub" => Format::Epub,
        "cbr" => Format::Cbr,
        "cbz" => Format::Cbz,
        "pdf" => Format::Pdf,
        "txt" => Format::Txt,
        _ => Format::Other,
    };

    // construct
    FileInfo {
        // TODO default id ? 🤮
        id: "666".to_string(),
        name: filename,
        library_name: library_name.to_string(),
        parent_path,
        added_date: since_the_epoch.as_secs() as i64,
        scan_me: 1,
        format,
        size: size.unwrap_or(0) as i64,
        total_pages: 0,
        read_by: "".to_string(),
        bookmarked_by: "".to_string(),
    }
}

/// walk library dir and return list of files modified after the last successfull scan
/// directory updated match new file, removed file
async fn walk_recent_dir(
    library_path: &Path,
    last_successfull_scan_date: Duration,
    conn: &Pool<Sqlite>,
) {
    // ) -> WalkDirGeneric<(usize, bool)> {
    debug!("start walkdir for recent directories");
    let updated_dir_list = WalkDirGeneric::<(usize, bool)>::new(library_path)
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
                            "modified time {}, greater than last successfull scan {} for directory [{}]",
                            dir_entry_modified_date.as_secs(),
                            last_successfull_scan_date.as_secs(),
                            dir_entry.file_name().to_string_lossy()
                        );
                        // flag dir for scan
                        dir_entry.client_state = true;
                    }
                }
            });
        });

    // loop on modified dirs
    for entry in updated_dir_list.into_iter().flatten() {
        if entry.client_state {
            let current_directory = DirectoryInfo {
                id: "666".to_string(),
                name: entry.file_name.to_string_lossy().to_string(),
                parent_path: entry.parent_path.to_string_lossy().to_string(),
                file_count: None,
            };
            info!(
                "new changes in dir \"{}/{}\", need to scan it",
                current_directory.parent_path, current_directory.name,
            );
            let directory_found = sqlite::check_if_directory_exists(
                &current_directory.parent_path,
                &current_directory.name,
                conn,
            )
            .await;
            // new directory
            if directory_found.is_empty() && !current_directory.parent_path.is_empty() {
                sqlite::insert_new_dir(&current_directory, None, conn).await;
            }
            // search for removed files
            // retrieve file list in database for current directory
            let registered_files = sqlite::get_files_from_directory(
                &entry.parent_path.to_string_lossy(),
                &entry.file_name.to_string_lossy(),
                conn,
            )
            .await;
            // check if files exists for current directory, delete in database if not
            for file in registered_files {
                let full_path = format!("{}/{}", file.parent_path, file.name);
                let file_path = Path::new(&full_path);
                if !file_path.is_file() {
                    sqlite::delete_file(&file, conn).await;
                }
            }
        }
    }
}

/// walk library dir and return list of files modified after the last successfull scan
/// insert them in the process_read_dir fn of jwalk crate
fn walk_recent_files_and_insert(library: Library, last_successfull_scan_date: Duration) {
    let library_path = Path::new(&library.path);
    // recursive walk_dir
    let recent_file_list = WalkDirGeneric::<(usize, bool)>::new(library_path)
        // TODO conf param
        .skip_hidden(true)
        .process_read_dir(move |_depth, _path, _read_dir_state, children| {
            children.iter_mut().for_each(|files_found| {
                if let Ok(file) = files_found {
                    // retrieve metadatas for ctime
                    let meta = fs::metadata(file.path()).unwrap();
                    let file_modified_date = Duration::from_secs(meta.st_ctime() as u64);
                    // check ctime for files only, because directories will be not crossed
                    // without this check
                    if file.file_type().is_file() && file_modified_date > last_successfull_scan_date
                    {
                        debug!(
                            "modified time {}, greater than last successfull scan {} for file [{}]",
                            file_modified_date.as_secs(),
                            last_successfull_scan_date.as_secs(),
                            file.file_name().to_string_lossy()
                        );
                        // insert here for the jwalk parallelism benefit
                        // file_infos need to be mutable for ulid genereation at the insert step
                        let mut file_infos =
                            extract_file_infos(&library.name, file.path().as_path());
                        let filename = file_infos.name.clone();
                        let parent_path = file_infos.parent_path.clone();
                        // create a new tokio runtime for inserts
                        // we need it to use async fns create_sqlite_pool and insert_new_file
                        // TODO create one conn for each insert ? not sure if it's realy optimal...
                        match Runtime::new() {
                            Ok(rt) => {
                                rt.block_on(async move {
                                    if let Ok(conn) = sqlite::create_sqlite_pool().await {
                                        // check if file alrdeady exists in database
                                        let file_found = sqlite::check_if_file_exists(
                                            parent_path.as_str(),
                                            filename.as_str(),
                                            &conn,
                                        )
                                        .await;
                                        if file_found.is_empty() {
                                            // new file
                                            info!("new file found : {}/{}", parent_path, filename);
                                            sqlite::insert_new_file(&mut file_infos, None, &conn)
                                                .await;
                                        } else if file_found.len() == 1 {
                                            // 1 file found, ok update it
                                            info!("file modified : {}/{}", parent_path, filename);
                                            let ulid_found = &file_found[0].id;
                                            // we dont want to loose flags
                                            file_infos.bookmarked_by =
                                                file_found[0].bookmarked_by.clone();
                                            file_infos.read_by = file_found[0].read_by.clone();
                                            // insert with up to date values
                                            sqlite::insert_new_file(
                                                &mut file_infos,
                                                Some(ulid_found),
                                                &conn,
                                            )
                                            .await;
                                        } else {
                                            // multiple id for a file ? should not happen !
                                            // TODO propose repair or full rescan
                                            error!(
                                        "base possibly corrupted, multiple id found for file \"{}/{}\"",
                                        parent_path, filename
                                    );
                                        }
                                    }
                                });
                                // flag file for insert
                                file.client_state = true;
                            }
                            Err(e) => {
                                error!("unable to create runtime for new file insertion : {e}")
                            }
                        }
                    }
                }
            });
        });
    for entry in recent_file_list.into_iter().flatten() {
        if entry.client_state {
            debug!(
                "insert file \"{}/{}\"",
                entry.parent_path.to_string_lossy(),
                entry.file_name.to_string_lossy()
            );
        }
    }
}

/// get files who need to be scanned (field `scan_me` in database) and extract some informations
pub async fn extraction_routine(speed: i32, sleep_time: Duration) {
    // wait a few seconds before start, let some time to the scan routine to add some files
    tokio::time::sleep(Duration::from_secs(10)).await;

    // create a database connection and start main loop
    match sqlite::create_sqlite_pool().await {
        Ok(conn) => {
            loop {
                info!("start extraction");

                // directories extract
                // we want file number in each directory
                let directories_to_scan_list: Vec<DirectoryInfo> =
                    match sqlx::query_as("SELECT * FROM directories;")
                        .fetch_all(&conn)
                        .await
                    {
                        Ok(directory) => directory,
                        Err(e) => {
                            error!("unable to retrieve directory list to scan : {e}");
                            let empty_list: Vec<DirectoryInfo> = Vec::new();
                            empty_list
                        }
                    };
                for directory in directories_to_scan_list {
                    let directory_full_path =
                        &format!("{}/{}", directory.parent_path, directory.name);
                    // TODO comments why `(i32,)` ??
                    // see https://github.com/launchbadge/sqlx/issues/1066 for example
                    let directory_file_count: (i32,) = match sqlx::query_as(
                        "SELECT count(*) FROM files WHERE instr(parent_path, ?) > 0;",
                    )
                    .bind(directory_full_path)
                    .fetch_one(&conn)
                    .await
                    {
                        Ok(file_count) => file_count,
                        Err(e) => {
                            error!(
                                "unable to retrieve file number for directory [{}] : {e}",
                                directory_full_path
                            );
                            (0,)
                        }
                    };
                    // TODO Move to `sqlite` mod
                    // insert number
                    match sqlx::query("UPDATE directories SET file_count = ? WHERE id = ?;")
                        .bind(directory_file_count.0)
                        .bind(directory.id)
                        .execute(&conn)
                        .await
                    {
                        Ok(_) => debug!(
                            "insert file count {} for directory [{}]",
                            directory_file_count.0, directory_full_path
                        ),
                        Err(e) => error!(
                            "unable to set file count for directory [{}] : {e}",
                            directory_full_path
                        ),
                    }
                }

                // files extract
                // TODO set extraction limit in conf ? (extraction speed)
                let files_to_scan_list: Vec<FileInfo> =
                    match sqlx::query_as("SELECT * FROM files WHERE scan_me = '1' LIMIT ?;")
                        .bind(speed)
                        .fetch_all(&conn)
                        .await
                    {
                        Ok(file_found) => file_found,
                        Err(e) => {
                            error!("unable to retrieve file list to scan : {e}");
                            let empty_list: Vec<FileInfo> = Vec::new();
                            empty_list
                        }
                    };
                if files_to_scan_list.is_empty() {
                    info!("0 file need to be scanned")
                }

                for file_to_scan in files_to_scan_list {
                    extract_all(&file_to_scan, &conn).await;
                }
                // TODO true schedule, last extract status in db...
                info!(
                    "stop extraction, sleeping for {} seconds",
                    sleep_time.as_secs()
                );
                tokio::time::sleep(sleep_time).await;
            }
        }
        Err(_) => error!("unable to start extraction routine"),
    }
}

async fn purge_removed_directories(conn: &Pool<Sqlite>) {
    // removed directory
    // TODO make a fn
    let registered_directories = sqlite::get_registered_directories(conn).await;
    // check if files exists for current directory, delete in database if not
    for directory in registered_directories {
        let full_path = format!("{}/{}", directory.parent_path, directory.name);
        let directory_path = Path::new(&full_path);
        if !directory_path.is_dir() {
            info!(
                "directory [{}] not found but still present in database, deleting",
                full_path
            );
            sqlite::delete_directory(&directory, conn).await;
        }
    }
}

/// scan function for routine or on demand
pub async fn launch_scan(library: &Library, conn: &Pool<Sqlite>) -> Result<()> {
    let library_path = Path::new(&library.path);

    if !library_path.is_dir() {
        error!("[{}] does not exists", library_path.to_string_lossy());
    } else {
        debug!(
            "path [{}] found and is a directory",
            library_path.to_string_lossy()
        );

        // check if scan is locked
        let scan_lock = sqlite::get_scan_lock(library, conn).await?;
        if scan_lock {
            info!(
                "library [{}] scan locked, already in progress",
                library.name
            );
        } else {
            // lock scan
            sqlite::toggle_scan_lock(library, conn).await?;

            // TODO remove this shit
            // 🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥
            // error!("sleep for {}", library.name);
            // let sleep_time = Duration::from_secs(20);
            // tokio::time::sleep(sleep_time).await;
            // error!("end sleep for {}", library.name);

            // TODO really need this ?
            // retrieve last_successfull_scan_date, 0 if first run
            // let last_successfull_scan_date = if first_scan_run {
            //     first_scan_run = false;
            //     Duration::from_secs(0)
            // } else {
            //     sqlite::get_last_successfull_scan_date(library_id, conn).await
            // };
            let last_successfull_scan_date =
                sqlite::get_last_successfull_scan_date(library.id, conn).await;
            debug!("last_successfull_scan_date : {last_successfull_scan_date:?}");

            // recent directories to find new and removed files
            walk_recent_dir(library_path, last_successfull_scan_date, conn).await;

            // removed directory
            purge_removed_directories(conn).await;

            // recent files : added and modified files
            // TODO this fn create a proper sql connexion, better this way ?
            walk_recent_files_and_insert(library.clone(), last_successfull_scan_date);

            // update file_count
            sqlite::update_library_file_count(library, conn).await;

            // end scanner, update date if successfull
            // TODO how to check if successfull ?
            // le at_least_one_insert_or_delete est pas bon car si rien change, c'est ok
            sqlite::update_last_successfull_scan_date(&library.id, conn).await;

            // lock scan
            sqlite::toggle_scan_lock(library, conn).await?;
        }
    }
    Ok(())
}

/// scan library path and add files in db
// batch insert -> no speed improvement
// TODO check total number file found, vs total in db (for insert errors) ?
pub async fn scan_routine(sleep_time: Duration) {
    match sqlite::create_sqlite_pool().await {
        Ok(conn) => {
            // reset scan_lock for all libraries (in case of previous crash)
            // TODO error handling
            let _ = sqlite::reset_scan_lock(&conn).await;

            // main loop
            loop {
                // retrieve library list at each run (if added from web ui...)
                let library_list = sqlite::get_library(None, None, &conn).await;

                // library path loop
                for library in library_list {
                    info!("start scan for library [{}]", &library.name);
                    // TODO error handling
                    let _ = launch_scan(&library, &conn).await;
                    info!("finish scan for library [{}]", &library.name);
                }

                // TODO true schedule, last scan status in db...
                debug!(
                    "stop scanning, sleeping for {} seconds",
                    sleep_time.as_secs()
                );
                tokio::time::sleep(sleep_time).await;
            }
        }
        Err(_) => error!("unable to start scan routine"),
    }
}

pub fn dynamic_image_to_vec_u8(image: DynamicImage) -> Option<Vec<u8>> {
    let mut bytes_comic_page: Vec<u8> = Vec::new();
    let mut writer = Cursor::new(&mut bytes_comic_page);
    let jpeg_encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, 75);
    // Jpeg does not support the color type `Rgba8`
    match image.into_rgb8().write_with_encoder(jpeg_encoder) {
        Ok(_) => {
            let vec_u8_image: &Vec<u8> = writer.get_ref();
            Some(vec_u8_image.to_owned())
        }
        Err(e) => {
            warn!("unable to convert cover to jpeg : {e}");
            None
        }
    }
}

pub async fn extract_all(file: &FileInfo, conn: &Pool<Sqlite>) {
    // cover
    let dynamic_image_cover = match file.format.as_str() {
        "epub" => extract_epub_cover(file),
        "pdf" => extract_pdf_cover(file),
        "cbz" | "cbr" | "cb7" => extract_comic_cover(file),
        _ => None,
    };

    if let Some(cover) = dynamic_image_cover {
        match dynamic_image_to_vec_u8(cover) {
            Some(buffered_u8_cover) => sqlite::insert_cover(file, &buffered_u8_cover, conn).await,
            None => warn!(
                "unable to insert cover for file {},{}",
                file.parent_path, file.name
            ),
        }
    }
    // total_pages
    match file.format.as_str() {
        "epub" => extract_epub_page_number(file, conn).await,
        "pdf" => extract_pdf_page_number(file, conn).await,
        "cbz" | "cbr" | "cb7" => extract_comic_page_number(file, conn).await,
        _ => (),
    }
    // scan_flag
    sqlite::set_scan_flag(file, 0, conn).await;
}

pub async fn extract_pdf_page_number(file: &FileInfo, conn: &Pool<Sqlite>) {
    let full_path = format!("file://{}/{}", file.parent_path, file.name);
    let mut total_pages = 0;
    // no await in this scope, cause error trait `Handler<_, _, _>` is not implemented
    // use macro #[axum::debug_handler] on handler to see details
    if let Ok(pdf_document) = Document::from_file(&full_path, None) {
        total_pages = pdf_document.n_pages();
    };
    sqlite::insert_total_pages(file, total_pages, conn).await;
}

// TODO error/warn message for each `None` in arms
// ⚠️  "the image crate is known to be quite slow when compiled in debug mode"
// from https://www.reddit.com/r/rust/comments/k1wjix/why_opening_of_images_is_so_slow/
pub fn extract_pdf_cover(file: &FileInfo) -> Option<image::DynamicImage> {
    // poppler-rs need an URI for file, so I prefix it with `file://`
    // TODO check with relative path ? => or force absolute ?
    let full_path = format!("file://{}/{}", file.parent_path, file.name);
    // cairo test, see https://github.com/DMSrs/poppler-rs/blob/master/src/lib.rs#L144
    // create a Write buffer to store surface
    let maybe_cover = Cursor::new(Vec::new());
    let surface = cairo::PdfSurface::for_stream(420.0, 595.0, maybe_cover).ok()?;
    let ctx = Context::new(&surface).ok()?;
    // open pdf file
    let pdf_document = Document::from_file(&full_path, None).ok()?;
    // get cover content and render it in surface
    let page = pdf_document.page(0)?;
    let (w, h) = page.size();
    surface.set_size(w, h).ok()?;
    ctx.save().ok()?;
    page.render(&ctx);
    // write surface in a new bytes Vec, it's the cover image
    let mut image_data: Vec<u8> = Vec::new();
    surface
        .write_to_png(&mut Cursor::new(&mut image_data))
        .ok()?;
    let cover = image::load_from_memory(&image_data).ok()?;
    // resize and go
    let resized_cover: Option<image::DynamicImage> = Some(resize_cover(cover));
    resized_cover
}

pub async fn extract_epub_page_number(file: &FileInfo, conn: &Pool<Sqlite>) {
    let full_path = format!("{}/{}", file.parent_path, file.name);
    if let Ok(doc) = EpubDoc::new(&full_path) {
        let total_pages = doc.get_num_chapters();
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

    // // encoding prb
    // // T03 - La Marque Des Demons.cbz
    // // unable to extract file list form archive : invalid utf-8 sequence of 1 bytes from index 20
    // // thread 'tokio-runtime-worker' panicked at 'get file path from file list at', src/reader.rs:26:18
    // // following code does not work, must find the correct encoding
    // // --------------
    // use encoding_rs::{GBK, SHIFT_JIS};
    // let decode_gbk = |bytes: &[u8]| {
    //     GBK.decode_without_bom_handling_and_without_replacement(bytes)
    //         .map(String::from)
    //         .ok_or(Error::Encoding(std::borrow::Cow::Borrowed("GBK failure")))
    // };
    // let decode_sjis = |bytes: &[u8]| {
    //     SHIFT_JIS
    //         .decode_without_bom_handling_and_without_replacement(bytes)
    //         .map(String::from)
    //         .ok_or(Error::Encoding(std::borrow::Cow::Borrowed(
    //             "SHIFT_JIS failure",
    //         )))
    // };
    // // let decode_utf8 = |bytes: &[u8]| Ok(std::str::from_utf8(bytes)?.to_owned());
    // let file_list = list_archive_files_with_encoding(archive, decode_sjis).expect("MYTEST");
    // use std::ffi::OsStr;
    // use std::os::unix::ffi::OsStrExt;
    // let mut comic_file_list: Vec<String> = Vec::new();
    // for file in file_list {
    //     let vecu8 = file.into_bytes();
    //     let os_str = OsStr::from_bytes(&vecu8[..]);
    //     let pathh = os_str.to_string_lossy().into_owned();
    //     comic_file_list.push(pathh);
    //     // error!("pathh : {pathh}");
    // }

    let comic_file_list = match list_archive_files(archive) {
        Ok(list) => list,
        Err(e) => {
            // 🔥🔥🔥 TODO 🔥🔥🔥 probably an encoding prb, error to warn (or info), and try with
            // list_archive_files_with_encoding ?
            // or ArchiveIterator...
            // ---------------------------------
            // let mut name = String::default();
            // let mut size = 0;
            // error!("start decode_utf8");
            // let decode_utf8 = |bytes: &[u8]| Ok(std::str::from_utf8(bytes)?.to_owned());
            // error!("start ArchiveIterator");
            // let mut iter = ArchiveIterator::from_read_with_encoding(archive, decode_utf8).unwrap();
            // error!("start loop");
            // for content in &mut iter {
            //     match content {
            //         ArchiveContents::StartOfEntry(s, _) => {
            //             error!("{s}");
            //             name = s
            //         }
            //         ArchiveContents::DataChunk(v) => size += v.len(),
            //         ArchiveContents::EndOfEntry => {
            //             println!("Entry {} was {} bytes", name, size);
            //             size = 0;
            //         }
            //         ArchiveContents::Err(e) => {
            //             error!("ArchiveContents Err : {e}");
            //         }
            //     }
            // }
            // error!("close");
            // iter.close().unwrap();

            warn!("unable to extract file list form archive : {e}");
            Vec::with_capacity(0)
        }
    };
    // TODO use drain_filter when it will be stable
    // see https://github.com/rust-lang/rust/issues/43244
    let mut image_list = Vec::with_capacity(comic_file_list.capacity());
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
    if let Ok(compressed_comic_file) = File::open(format!("{}/{}", file.parent_path, file.name)) {
        let file_list = extract_comic_image_list(&compressed_comic_file);
        let total_pages = file_list.len();
        sqlite::insert_total_pages(file, total_pages as i32, conn).await;
    }
}

pub fn extract_comic_cover(file: &FileInfo) -> Option<image::DynamicImage> {
    let archive_path = &format!("{}/{}", file.parent_path, file.name);
    if let Ok(compressed_comic_file) = File::open(archive_path) {
        // get images list from archive
        let comic_file_list = extract_comic_image_list(&compressed_comic_file);
        // set path file wanted from page index
        let image_path_in_achive = match comic_file_list.first() {
            Some(path) => path,
            None => {
                warn!("could not retrive cover in archive");
                ""
            }
        };
        // uncompress corresponding image
        let mut vec_cover: Vec<u8> = Vec::new();
        // RAR need to reopen file... why ? and why rar only ?
        match File::open(archive_path) {
            Ok(compressed_comic_file) => {
                // ⚠ unsafe code here from compress-tools
                // TODO change lib ?
                match uncompress_archive_file(
                    &compressed_comic_file,
                    &mut vec_cover,
                    image_path_in_achive,
                ) {
                    Ok(_) => (),
                    Err(e) => warn!(
                        "unable to extract path [{}] from file [{}] : {e}",
                        image_path_in_achive, file.name
                    ),
                }
            }
            Err(e) => {
                warn!(
                    "unable to open path [{}] from file [{}] : {e}",
                    image_path_in_achive, file.name
                );
            }
        };

        match image::load_from_memory(&vec_cover) {
            // match image::load_from_memory(&cover.0) {
            Ok(img) => {
                let cover = resize_cover(img);
                Some(cover)
            }
            Err(_) => {
                warn!("I can't decode cover image for file {archive_path}");
                None
            }
        }
    } else {
        None
    }
}

// TODO easy testing here...
pub fn resize_cover(cover: image::DynamicImage) -> image::DynamicImage {
    // see doc https://docs.rs/image/0.24.5/image/imageops/enum.FilterType.html
    // for quality of resize (Nearest is ugly)
    // TODO do not keep ratio ? crop ? the max heigh is the most important
    // TODO test thumbnail fn :
    // https://docs.rs/image/latest/image/enum.DynamicImage.html#method.thumbnail
    // cover.resize(180, 280, FilterType::Triangle) // moy 20 ko
    // cover.resize(360, 560, FilterType::Triangle) // moy 40 ko
    cover.resize(280, 430, FilterType::Triangle) // moy 60 ko

    // // test crate fast_image_resize ?
    // use fast_image_resize as fir;
    // use std::num::NonZeroU32;
    // let width = NonZeroU32::new(cover.width()).unwrap();
    // let height = NonZeroU32::new(cover.height()).unwrap();
    // let src_image = fir::Image::from_vec_u8(
    //     width,
    //     height,
    //     cover.to_rgb8().into_raw(),
    //     fir::PixelType::U8x3,
    // )
    // .unwrap();
    // let mut src_view: fir::DynamicImageView = src_image.view();
    // let dst_width = NonZeroU32::new(150).unwrap();
    // let dst_height = NonZeroU32::new(230).unwrap();
    // src_view.set_crop_box_to_fit_dst_size(dst_width, dst_height, None);
    // let mut dst_image = fir::Image::new(dst_width, dst_height, src_view.pixel_type());
    // let mut dst_view = dst_image.view_mut();
    // let mut resizer = fir::Resizer::new(fir::ResizeAlg::Convolution(fir::FilterType::Lanczos3));
    // resizer.resize(&src_view, &mut dst_view).unwrap();
    // let toto = dst_image.into_vec();
    // image::load_from_memory(&toto).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    // use crate::sqlite;
    // use sqlx::{migrate::MigrateDatabase, Sqlite};
    use std::io::prelude::*;

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
        let validation_file = extract_file_infos(
            "library",
            &library_path.join("Asterix/T01 - Asterix le Gaulois.pdf"),
        );
        insta::assert_yaml_snapshot!(validation_file, {
            ".added_date" => "[added_date]"
        });
        // delete library
        delete_fake_library(library_path).unwrap_or(());
    }

    // #[tokio::test]
    // async fn test_insert_new_file() {
    //     // init database
    //     let _ = sqlite::init_database().await;
    //     // run test
    //     let mut skeletion_file = FileInfo {
    //         // id is random, so we take it from validation_file
    //         id: "666".to_string(),
    //         name: "T01 - Asterix le Gaulois.pdf".to_string(),
    //         library_name: "library".to_string(),
    //         parent_path: "library/Asterix".to_string(),
    //         scan_me: 1,
    //         added_date: 0,
    //         format: "pdf".to_string(),
    //         size: 10,
    //         total_pages: 0,
    //         read_by: "".to_string(),
    //         bookmarked_by: "".to_string(),
    //     };
    //     match sqlite::create_sqlite_pool().await {
    //         Ok(conn) => {
    //             // main loop
    //             sqlite::insert_new_file(&mut skeletion_file, None, &conn).await;
    //             let file_from_base: Vec<FileInfo> =
    //                 match sqlx::query_as("SELECT * FROM files WHERE parent_path = ?;")
    //                     .bind(&skeletion_file.parent_path)
    //                     .fetch_all(&conn)
    //                     .await
    //                 {
    //                     Ok(file_found) => file_found,
    //                     Err(e) => {
    //                         error!("unable to insert file infos from database : {}", e);
    //                         let empty_list: Vec<FileInfo> = Vec::new();
    //                         empty_list
    //                     }
    //                 };
    //             assert_eq!(file_from_base.first().unwrap().name, skeletion_file.name);
    //         }
    //         Err(_) => error!("unable to start scan routine"),
    //     }
    //     // delete database
    //     let _ = Sqlite::drop_database(crate::DB_URL).await;
    // }

    // #[tokio::test]
    // async fn test_walkdir() {
    //     let conn = sqlite::create_sqlite_pool().await;
    //     // create library
    //     let library_path = Path::new("library_walkdir");
    //     create_fake_library(library_path).unwrap_or(());
    //     // run test
    //     let timestamp_flag = Duration::from_secs(666);
    //     // recent directories
    //     let dir_list = walk_recent_dir(library_path, timestamp_flag, &conn).await;
    //     // delete database
    //     Sqlite::drop_database(crate::DB_URL);
    //     let mut dir_list_path: Vec<String> = vec![];
    //     for entry in dir_list.into_iter().flatten() {
    //         if entry.client_state {
    //             dir_list_path.push(entry.path().to_string_lossy().to_string());
    //         }
    //     }
    //     let mut check_dir_list_path: Vec<String> = vec![
    //         "library_walkdir".to_string(),
    //         "library_walkdir/Asterix".to_string(),
    //         "library_walkdir/Goblin's".to_string(),
    //         "library_walkdir/H.P. Lovecraft".to_string(),
    //         "library_walkdir/H.P. Lovecraft/Le Cauchemar d'Innsmouth (310)".to_string(),
    //         "library_walkdir/Dragonlance".to_string(),
    //     ];
    //     dir_list_path.sort();
    //     check_dir_list_path.sort();
    //     assert_eq!(dir_list_path, check_dir_list_path);
    //     // recent files
    //     let file_list = walk_recent_files_and_insert(library_path, timestamp_flag);
    //     let mut file_list_path: Vec<String> = vec![];
    //     for entry in file_list.into_iter().flatten() {
    //         if entry.client_state {
    //             file_list_path.push(entry.path().to_string_lossy().to_string());
    //         }
    //     }
    //     let mut check_file_list_path: Vec<String> = vec![
    //         "library_walkdir/Asterix/T01 - Asterix le Gaulois.pdf".to_string(),
    //         "library_walkdir/Asterix/T02 - La Serpe d'Or.pdf".to_string(),
    //         "library_walkdir/Goblin's/T01.cbz".to_string(),
    //         "library_walkdir/Goblin's/T02.cbz".to_string(),
    //         "library_walkdir/H.P. Lovecraft/Le Cauchemar d'Innsmouth (310)/cover.jpg".to_string(),
    //         "library_walkdir/H.P. Lovecraft/Le Cauchemar d'Innsmouth (310)/metadata.opf".to_string(),
    //         "library_walkdir/H.P. Lovecraft/Le Cauchemar d'Innsmouth (310)/Le Cauchemar d'Innsmouth - Howard Phillips Lovecraft.epub".to_string(),
    //     ];
    //     file_list_path.sort();
    //     check_file_list_path.sort();
    //     assert_eq!(file_list_path, check_file_list_path);
    //     // delete database
    //     delete_fake_library(library_path).unwrap_or(());
    // }

    // #[test]
    // fn test_delete_file() {
    //     // TODO
    //     todo!();
    // }
}
