use crate::scanner::{self, FileInfo};
use crate::sqlite;

use compress_tools::*;
use epub::doc::EpubDoc;
use std::fs::File;

// pub fn raw(_user: User, _file: FileInfo) -> String {
//     todo!();
// }
// pub fn txt(_user: User, _file: FileInfo) -> String {
//     todo!();
// }

// TODO load previous and next images for smoother experience ?
pub async fn comics(file: &FileInfo, page: i32) -> String {
    info!("reading {}/{} (page {page})", file.parent_path, file.name);
    // mark as read if last page
    if page == file.total_pages {
        println!("toto");
    }

    let archive_path = &format!("{}/{}", file.parent_path, file.name);
    match File::open(archive_path) {
        Ok(compressed_comic_file) => {
            // get images list from archive
            let comic_file_list = scanner::extract_comic_image_list(&compressed_comic_file);
            // set path file wanted from page index
            let image_path_in_achive = comic_file_list
                .get(page as usize)
                .expect("get file path from file list at");
            // uncompress corresponding image
            let mut vec_comic_page: Vec<u8> = Vec::default();

            // RAR need to reopen file... why ? and why rar only ?
            let compressed_comic_file = File::open(archive_path).expect("file open");
            match uncompress_archive_file(
                &compressed_comic_file,
                &mut vec_comic_page,
                image_path_in_achive,
            ) {
                Ok(_) => (),
                Err(e) => error!(
                    "unable to extract path '{}' from file '{}' : {e}",
                    image_path_in_achive, file.name
                ),
            }
            // return img in base64
            match image::load_from_memory(&vec_comic_page) {
                Ok(img) => {
                    format!(
                        // TODO create a `page` router to render directly without base64
                        "<img src=\"data:image/jpeg;base64,{}\" class=\"responsive\")",
                        sqlite::image_to_base64(&img)
                    )
                }
                Err(_) => "error comic".to_string(),
            }
        }
        Err(e) => {
            error!("unable to read file {archive_path} : {e}");
            String::new()
        }
    }
}

pub async fn epub(file: &FileInfo, page: i32) -> String {
    // open file
    let full_path = format!("{}/{}", file.parent_path, file.name);
    let mut doc = match EpubDoc::new(full_path) {
        Ok(doc) => doc,
        // TODO true error handling...
        // suffit d'inclure le reste du code dans le arm Ok...
        Err(_) => EpubDoc::new("toto").unwrap(),
    };
    // set page at current_page
    doc.set_current_page(page as usize);
    // title
    let title = if let Some(title) = doc.mdata("title") {
        title
    } else {
        "Book title not found".to_string()
    };
    // pages
    let total_pages = doc.get_num_pages();
    // resources
    let toto = match doc.get_current_id() {
        Some(toto) => toto,
        None => "toto".to_string(),
    };
    // add css
    // let extracss = "body { background-color: #303030; color: white }";
    // doc.add_extra_css(extracss);

    let mut reader = format!(
        "page: {}/{}<br />
         fs path: {}-{}<br />
         <h1>title: {}</h1>
        ",
        page, total_pages, file.parent_path, file.name, title,
    );
    for _ in 0..total_pages {
        let current_page = doc.get_current_page();
        // let (page_content, _) = doc.get_current_str().unwrap();
        let page_content = match doc.get_current_with_epub_uris() {
            Ok(content) => String::from_utf8(content).expect("pas utf8"),
            Err(_) => doc.get_current_str().unwrap().0,
        };

        reader.push_str(&format!(
            "<h2>resource: {}/{}</h2>
             toto: {}<br />
             content: {}<br />
             <br /><br />
            ",
            current_page, total_pages, toto, page_content
        ));
        doc.go_next();
    }
    reader
}
