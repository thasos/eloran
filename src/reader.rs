use crate::scanner::FileInfo;
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
    info!("reading {}/{} (page {page}", file.parent_path, file.name);
    let compressed_comic_file =
        File::open(format!("{}/{}", file.parent_path, file.name)).expect("file open");
    // the fn uncompress_archive_file from crate compress_tools does not work here with all files
    // (KO with CBR), but it works with ArchiveIterator
    let mut comic_iter = ArchiveIterator::from_read(&compressed_comic_file).expect("iterator");
    let mut file_path_in_archive = String::default();
    let mut vec_comic_page: Vec<u8> = Vec::default();
    // the ArchiveIterator index does not fit the files index in archive so I have to create my own
    let mut index: usize = 0;
    for content in &mut comic_iter {
        match content {
            ArchiveContents::StartOfEntry(s, _) => file_path_in_archive = s,
            ArchiveContents::DataChunk(vec_chunk) => {
                // add chunks in the image Vec
                if index == page as usize {
                    for chunk in vec_chunk {
                        vec_comic_page.push(chunk);
                    }
                }
            }
            ArchiveContents::EndOfEntry => {
                // increase index in case of new file
                index += 1;
            }
            ArchiveContents::Err(e) => {
                error!(
                    "can't extract path {} in comic file {}/{} {e}",
                    file_path_in_archive, file.parent_path, file.name
                );
            }
        }
    }
    comic_iter.close().unwrap();
    // return img in base64
    match image::load_from_memory(&vec_comic_page) {
        Ok(img) => {
            format!(
                // TODO create a `page` router to render directly (no b64)
                "<img src=\"data:image/jpeg;base64,{}\")",
                sqlite::image_to_base64(&img)
            )
        }
        Err(_) => "error comic".to_string(),
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
