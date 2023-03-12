use crate::scanner::FileInfo;

use epub::doc::EpubDoc;

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
// pub fn cb7(_user: User, _file: FileInfo) -> String {
//     todo!();
// }
// pub fn txt(_user: User, _file: FileInfo) -> String {
//     todo!();
// }

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

    // <p><img src=\"/covers/id/cover.jpg\"/></p>
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
