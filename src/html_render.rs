use crate::scanner::FileInfo;
use crate::{http_server::User, scanner::DirectoryInfo};
use horrorshow::{helper::doctype, Template};

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
                : "diskpath = ";
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
                div(class="gallery") {
                    a(href=format!("/library{}/{}", &current_path, &directory.directory_name)) {
                        img(src="/images/folder.svgz", alt="folder", width="600", height="400")
                        : format_args!("{}", directory.directory_name)
                    }
                }
            }
            @ for file in &files_list {
                div(class="gallery") {
                    a(href=format!("/read/{}/{}", &current_path, &file.filename)) {
                        img(src="/images/green_book.svgz", alt="green book", width="600", height="400")
                        : format_args!("{}", file.filename)
                    }
                }
            }
        }
    };
    render(body_content)
}

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
