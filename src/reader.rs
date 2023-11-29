use crate::scanner::{self, FileInfo};

use compress_tools::*;
use epub::doc::EpubDoc;
use image::imageops::FilterType;
use std::fs::File;
use std::io::Cursor;

// pub fn raw(_user: User, _file: FileInfo) -> String {
//     todo!();
// }
// pub fn txt(_user: User, _file: FileInfo) -> String {
//     todo!();
// }

pub async fn get_comic_page(file: &FileInfo, page: i32, size: &str) -> Option<Vec<u8>> {
    info!(
        "reading comic {}/{} (page {page})",
        file.parent_path, file.name
    );
    let archive_path = &format!("{}/{}", file.parent_path, file.name);
    match File::open(archive_path) {
        Ok(compressed_comic_file) => {
            // get images list from archive
            let comic_file_list = scanner::extract_comic_image_list(&compressed_comic_file);
            // set path file wanted from page index
            if !comic_file_list.is_empty() {
                let image_path_in_achive = comic_file_list.get(page as usize)?;
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
                    Err(e) => warn!(
                        "unable to extract path '{}' from file '{}' : {e}",
                        image_path_in_achive, file.name
                    ),
                }
                // return img in jpg
                let dyn_image_comic_page = image::load_from_memory(&vec_comic_page).ok()?;
                // resize smaller if needed
                let dyn_image_comic_page = match size {
                    // TODO true ratio not needed, but check size (600 px too much ?)
                    // let w = dyn_image_comic_page.width();
                    // let h = dyn_image_comic_page.height();
                    "800px" => dyn_image_comic_page.resize(800, 2000, FilterType::Triangle),
                    "1000px" => dyn_image_comic_page.resize(1000, 2500, FilterType::Triangle),
                    _ => dyn_image_comic_page,
                };
                // encode to jpeg
                // TODO do not encode if already jpeg
                let mut bytes_comic_page: Vec<u8> = Vec::new();
                dyn_image_comic_page
                    .write_to(
                        &mut Cursor::new(&mut bytes_comic_page),
                        // jpeg quality
                        image::ImageOutputFormat::Jpeg(75),
                    )
                    .ok()?;
                Some(bytes_comic_page)
            } else {
                None
            }
        }
        Err(_) => None,
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

// // TODO create a non empty file for testing
// #[cfg(test)]
// mod tests {
//     use super::*;
//     #[tokio::test]
//     async fn test_get_comic_page() {
//         let file = FileInfo::default();
//         let page: i32 = 10;
//         let size = "123456";
//         insta::assert_yaml_snapshot!(get_comic_page(&file, page, size).await)
//     }
//     #[tokio::test]
//     async fn test_epub() {
//         let file = FileInfo::default();
//         let page: i32 = 10;
//         insta::assert_yaml_snapshot!(epub(&file, page).await)
//     }
// }
