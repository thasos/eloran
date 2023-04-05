use crate::scanner::FileInfo;
use crate::{http_server::User, scanner::DirectoryInfo};

use horrorshow::{helper::doctype, Raw, Template};

fn header<'a>(redirect_url: Option<&'a str>) -> Box<dyn horrorshow::RenderBox + 'a> {
    box_html! {
        title : "Eloran";
        meta(charset="UTF-8");
        meta(name="viewport", content="width=device-width");
        link(rel="stylesheet", href="/css/eloran.css");
        link(rel="stylesheet", href="/css/w3.css");
        link(rel="stylesheet", href="/css/gallery.css");
        link(rel="stylesheet", href="/css/w3-theme-dark-grey.css");
        meta(http-equiv="Cache-Control", content="no-cache, no-store, must-revalidate");
        meta(http-equiv="Pragma", content="no-cache");
        meta(http-equiv="Expires", content="0");
        // TODO do this better, it's awfull to me :(
        // add a meta tag with url to redirect
        : if let Some(url) = redirect_url {
            let redirect_timer = 0;
            let meta_redirect=format!("<meta http-equiv=\"refresh\" content=\"{redirect_timer}; url='{url}'\" />");
            Raw(meta_redirect)
        } else {
            // else cannot be (), and it's logic...
            Raw("".to_string())
        };
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

    render(body_content, None)
}

pub fn login_ok(user: &User) -> String {
    debug!("fn login ok");
    // TODO moche
    let user = user.clone();
    let body_content = box_html! {
        p { : format!("Successfully logged in as: {}, role {:?}", user.name.as_str(), &user.role) }
        p { a(href="/") : "return home" }
    };

    let redirect_url = "/library";
    render(body_content, Some(redirect_url))
}

pub fn logout(user: &User) -> String {
    debug!("fn logout");
    // TODO moche
    let user = user.clone();
    let body_content = box_html! { p
        { : format!("Bye {}", user.name.as_str()) }
        p { a(href="/") : "return home" }
    };

    let redirect_url = "/";
    render(body_content, Some(redirect_url))
}

pub fn file_info(
    user: &User,
    file: &FileInfo,
    bookmark_status: bool,
    read_status: bool,
    up_link: String,
) -> String {
    let menu = menu(user.clone());
    let file = file.clone();
    let body_content = box_html! {
        : menu;
        div(id="infos") {
            h2(style="text-align: center;") {
                a(href=format!("/read/{}/{}", file.id, file.current_page), class="navigation") : "📖 read file";
                : " | " ;
                a(href=format!("/download/{}", file.id), class="navigation") : "⤵ download";
            }
            h2 { a(href= up_link , class="navigation") : "↖️  up" }
            div(id="flags") {
                a(href=format!("/toggle/bookmark/{}", file.id)) : "toggle bookmarks";
                br;
                a(href=format!("/toggle/read_status/{}", file.id)) : "toggle read status";
            }
            div(id="infos") {
                h4(style="text-align: center;") {
                    : if bookmark_status { "⭐" } else { "" };
                    : if read_status { "✅" } else { "" };
                }
                h2(style="text-align: center;") { : file.name ; }
                img(src=format!("/cover/{}", file.id), alt="cover", width="150", height="230", class="infos");
                p(style="text-align: center;") {
                    : format!("size : {}", file.size) ;
                    br;
                    : format!("page : {}/{}",file.current_page, file.total_pages) ;
                    br;
                    : format!("type : {}",file.format) ;
                    br;
                    : format!("added : {}",file.added_date) ;
                }
            }
        }
    };
    render(body_content, None)
}

pub fn flag_toggle(user: &User, flag_status: bool, file_id: &str, flag: &str) -> String {
    let menu = menu(user.clone());
    let flag_response = match flag {
        "bookmark" => {
            if flag_status {
                "Bookmark added"
            } else {
                "Bookmark deleted"
            }
        }
        "read_status" => {
            if flag_status {
                "Marked as read"
            } else {
                "Marked as unread"
            }
        }
        _ => "",
    };
    let body_content = box_html! {
        : menu;
        div(id="toggle") {
            h2(style="text-align: center;") {
                : flag_response;
            }
        }
    };
    let redirect_url = format!("/infos/{file_id}");
    render(body_content, Some(&redirect_url))
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
    let next_page = if page < file.total_pages - 1 {
        page + 1
    } else {
        file.total_pages - 1
    };
    // add menu and nav links to ebook raw rendering
    let body_content = box_html! {
        : menu;
        h1(id="navigation", align="center") {
            // TODO go to page number
            a(href=format!("/read/{}/{}", file.id, previous_page), class="navigation") : "⬅️";
            : " | " ;
            a(href=format!("/read/{}/{}", file.id, 0), class="navigation") : "start";
            : " | " ;
            a(href=format!("/infos/{}", file.id), class="navigation") : "close";
            : " | " ;
            a(href=format!("/read/{}/{}", file.id, file.total_pages - 1), class="navigation") : "end";
            : " | " ;
            a(href=format!("/read/{}/{}", file.id, next_page), class="navigation") : "➡️";
        }
        div(id="epub-content") {
            p {: Raw(epub_content); }
        }
    };
    render(body_content, None)
}

pub fn library(
    user: &User,
    current_path: String,
    directories_list: Vec<DirectoryInfo>,
    files_list: Vec<(FileInfo, bool, bool)>,
    library_path: String,
) -> String {
    debug!("fn homepage");
    // TODO add comment
    let mut full_path: Vec<&str> = current_path.split('/').collect();
    full_path.pop();
    let mut parent_directory = String::new();
    // TODO better variable name
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
                : "disk path = ";
                // TODO split and add a direct link for each element in path
                : format!("{library_path}{}", &current_path);
            }
            h2 { a(href=format!("/library{}", &parent_directory), class="navigation") : "↖️  up" }
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
                    a(href=format!("/infos/{}", &file.0.id)) {
                        img(src=format!("/cover/{}", &file.0.id), alt="cover", width="150", height="230", class= if file.2 { "cover_read" } else { "cover" } )
                        : format_args!("{}", file.0.name);
                    }
                    h4(style="text-align: center;") {
                        : if file.1 { "⭐" } else { "" };
                        : if file.2 { "✅" } else { "" };
                    }
                }
            }
        }
    };
    render(body_content, None)
}

fn menu<'a>(user: User) -> Box<dyn horrorshow::RenderBox + 'a> {
    debug!("fn menu");
    // TODO print a pretty menu, 1 line...
    let menu_content = box_html! {
        div(id="menu") {
            p {
                a(href="/library") : "library" ;
                : " | ";
                a(href="/prefs") : "preferences" ;
                : " | ";
                : format!("{}", user.name.as_str()) ;
                : " (";
                a(href="/logout") : "logout" ;
                : ")";
            }
            form(action="/search", method="post") {
                input(type="text", placeholder="Search..", name="query") ;
            }
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
    render(body_content, None)
}

pub fn search_result(user: &User, files_list: Vec<FileInfo>) -> String {
    debug!("fn homepage");
    // TODO moche (obligé le clone  ?)
    let menu = menu(user.clone());
    let body_content = box_html! {
        : menu;
        div(id="results") {
            @ for file in &files_list {
                div(class="gallery", style="box-shadow: 0 4px 8px 0 rgba(0, 0, 0, 0.2), 0 6px 20px 0 rgba(0, 0, 0, 0.19);") {
                    a(href=format!("/infos/{}", &file.id)) {
                        img(src=format!("/cover/{}", &file.id), alt="cover", width="150", height="230")
                        : format_args!("{}", file.name);
                    }
                    // TODO as library display, need Vec<(FileInfo, bool, bool)>
                    // h4(style="text-align: center;") {
                    //     : if file.1 { "⭐" } else { "" };
                    //     : if file.2 { "✅" } else { "" };
                    // }
                }
            }
        }
    };
    render(body_content, None)
}

/// take body content html box, and return all the page with headers and full body
fn render(body_content: Box<dyn horrorshow::RenderBox>, redirect_url: Option<&str>) -> String {
    let full_page = html! { : doctype::HTML;
    html {
        head { : header(redirect_url); }
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
