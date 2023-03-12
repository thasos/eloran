use crate::scanner::FileInfo;
use crate::{http_server::User, scanner::DirectoryInfo};

use horrorshow::{helper::doctype, Raw, Template};

fn header<'a>() -> Box<dyn horrorshow::RenderBox + 'a> {
    // TODO css, metadatas...
    box_html! {
        title : "Eloran";
        meta(charset="UTF-8");
        meta(name="viewport", content="width=device-width");
        link(rel="stylesheet", href="/css/w3.css");
        link(rel="stylesheet", href="/css/gallery.css");
        link(rel="stylesheet", href="/css/w3-theme-dark-grey.css");
        meta(http-equiv="Cache-Control", content="no-cache, no-store, must-revalidate");
        meta(http-equiv="Pragma", content="no-cache");
        meta(http-equiv="Expires", content="0");
    }
}

pub fn login_form() -> String {
    debug!("fn login_form");
    let body_content = box_html! {
        p { : "Please login :" }
        p {
            form(action="/login", method="post") {
            input(type="text", name="user", placeholder="username", required);
            br;
            input(type="password", name="password", placeholder="password", required);
            br;
            input(type="submit", value="Login");
            }
        }
    };

    render(body_content)
}

// TODO auto return home (redirect ?)
pub fn login_ok(user: &User) -> String {
    debug!("fn login ok");
    // TODO moche
    let user = user.clone();
    let body_content = box_html! {
        p { : format!("Successfully logged in as: {}, role {:?}", user.name.as_str(), &user.role) }
        p { a(href="/") : "return home" }
    };

    render(body_content)
}

// TODO auto return home (redirect ?)
pub fn logout(user: &User) -> String {
    debug!("fn logout");
    // TODO moche
    let user = user.clone();
    let body_content = box_html! { p
        { : format!("Bye {}", user.name.as_str()) }
        p { a(href="/") : "return home" }
    };

    render(body_content)
}

pub fn file_info(user: &User, file: &FileInfo) -> String {
    let menu = menu(user.clone());
    let file = file.clone();
    let body_content = box_html! {
        : menu;
        div(id="infos") {
            h2(style="text-align: center;") {
                a(href=format!("/read/{}/{}", file.id, file.current_page)) : "read file";
            }
        }
    };
    render(body_content)
}

pub fn ebook_reader(user: &User, file: &FileInfo, epub_content: &str, page: i32) -> String {
    let menu = menu(user.clone());
    let epub_content = epub_content.to_string();
    let file = file.clone();
    // don't go outside the range of the book
    let previous_page = match page {
        0 => 0,
        _ => page - 1,
    };
    let next_page = if page < file.total_pages {
        page + 1
    } else {
        file.total_pages
    };
    // add menu and nav links to ebook raw rendering
    let body_content = box_html! {
        : menu;
        div(id="navigation") {
            a(href=format!("/read/{}/{}", file.id, previous_page)) : "<-";
            : " | " ;
            a(href=format!("/read/{}/{}", file.id, next_page)) : "->";
        }
        div(id="epub-content") {
            p {: Raw(epub_content); }
        }
    };
    render(body_content)
}

pub fn library(
    user: &User,
    current_path: String,
    directories_list: Vec<DirectoryInfo>,
    files_list: Vec<FileInfo>,
    library_path: String,
) -> String {
    debug!("fn homepage");
    let mut full_path: Vec<&str> = current_path.split('/').collect();
    full_path.pop();
    let mut parent_directory = String::new();
    for word in full_path {
        parent_directory.push_str(word);
        parent_directory.push('/');
    }
    parent_directory.pop();

    // TODO moche (obligé le clone  ?)
    let menu = menu(user.clone());
    let body_content = box_html! {
        : menu;
        div(id="library-content") {
            p {
                : "url path = ";
                : format!("/library{}", &current_path);
                br;
                : "disk path = ";
                : format!("{library_path}{}", &current_path);
            }
            p {
                    a(href=format!("/library{}", &parent_directory)) {
                        : "up"
                    }
            }
            // image gallery
            // https://www.w3schools.com/Css/css_image_gallery.asp
            @ for directory in &directories_list {
                div(class="gallery", style="box-shadow: 0 4px 8px 0 rgba(0, 0, 0, 0.2), 0 6px 20px 0 rgba(0, 0, 0, 0.19);") {
                    a(href=format!("/library{}/{}", &current_path, &directory.name)) {
                        img(src="/images/folder.svgz", alt="folder", width="150", height="230")
                        : format_args!("{}", directory.name)
                    }
                }
            }
            @ for file in &files_list {
                div(class="gallery", style="box-shadow: 0 4px 8px 0 rgba(0, 0, 0, 0.2), 0 6px 20px 0 rgba(0, 0, 0, 0.19);") {
                    // TODO add field `current page` instead of page 0...
                    a(href=format!("/infos/{}", &file.id)) {
                        // TODO if cover found, print it !
                        // img(src=format!("{}", display_cover(file)), alt="green book", width="150", height="230")
                        img(src=format!("/cover/{}", &file.id), alt="green book", width="150", height="230")
                        : format_args!("{}", file.name)
                    }
                }
            }
        }
    };
    render(body_content)
}

// fn display_cover(file: &FileInfo) -> String {
//     if file.cover.is_empty() {
//         "/images/green_book.svgz".to_string()
//     } else {
//         format!("data:image/jpeg;base64,{}", file.cover)
//         // TODO activate
//         // format!("/covers/{}", file.id)
//     }
// }

fn menu<'a>(user: User) -> Box<dyn horrorshow::RenderBox + 'a> {
    debug!("fn menu");
    // TODO sur 1 ligne...
    let menu_content = box_html! {
        div(id="menu") {
            p { : format!("Logged in as: {}, role {:?}", user.name.as_str(), &user.role) }
            p { a(href="/library") : "library" }
            p { a(href="/prefs") : "preferences" }
            p { a(href="/logout") : "logout" }
        }
    };
    menu_content
}

pub fn homepage(user: &User) -> String {
    debug!("fn homepage");
    // TODO moche (obligé le clone  ?)
    let menu = menu(user.clone());
    let body_content = box_html! {
        : menu;
        div(id="home-content") {
        : "content"
        }
    };
    render(body_content)
}

/// take body content html box, and return all the page with headers and full body
fn render(body_content: Box<dyn horrorshow::RenderBox>) -> String {
    let full_page = box_html! { : doctype::HTML;
    html {
        head { : header(); }
        body(class="w3-theme-dark") {
            h2(id="heading") { : "Welcome to Eloran" }
            : body_content
            }
        }
    };
    match full_page.into_string() {
        Ok(page) => page,
        // TODO true Error page (should not happen)
        Err(_) => "KO".to_string(),
    }
}
