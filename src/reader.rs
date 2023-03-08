use crate::http_server::User;
use crate::scanner::FileInfo;
use epub::doc::EpubDoc;
use image::ImageFormat;

// pub fn raw(_user: User, _file: FileInfo) -> String {
//     todo!();
// }
// pub fn pdf(_user: User, _file: FileInfo) -> String {
//     todo!();
// }
// pub fn cbz(_user: User, _file: FileInfo) -> String {
//     todo!();
// }
// pub fn cbr(_user: User, _file: FileInfo) -> String {
//     todo!();
// }
// pub fn txt(_user: User, _file: FileInfo) -> String {
//     todo!();
// }

// TODO handle error
pub fn _extract_epub_cover(_user: &User, file: &FileInfo) -> image::DynamicImage {
    let mut toto: image::DynamicImage = image::DynamicImage::new_luma8(0, 0);
    if file.format == "epub" {
        let full_path = format!("{}/{}", file.parent_path, file.name);
        let mut doc = match EpubDoc::new(full_path) {
            Ok(doc) => doc,
            // TODO true error handling...
            Err(_) => EpubDoc::new("toto").unwrap(),
        };
        // (Vec<u8>, String) : img and mime-type
        let cover = if let Some(cover) = doc.get_cover() {
            cover
        } else {
            (vec![], String::new())
        };
        // toto = image::load_from_memory_with_format(&cover.0, ImageFormat::Jpeg).unwrap();
        match image::load_from_memory_with_format(&cover.0, ImageFormat::Jpeg) {
            Ok(img) => toto = img,
            Err(_) => toto = image::DynamicImage::new_luma8(0, 0),
        };
    }
    toto
}

pub async fn epub(file: &FileInfo, page: i32) -> String {
    // open file
    let full_path = format!("{}/{}", file.parent_path, file.name);
    let mut doc = match EpubDoc::new(full_path) {
        Ok(doc) => doc,
        // TODO true error handling...
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

    // <p><img src=\"/covers/id/cover.jpg\"/></p>
    let mut reader = format!(
        "page: {}/{}<br />
         fs path: {}-{}<br />
         <h1>title: {}</h1>
        ",
        page, total_pages, file.parent_path, file.name, title,
    );
    for _toto in 0..total_pages {
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
