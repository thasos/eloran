use crate::http_server::Role;
use crate::http_server::User;
use crate::scanner::DirectoryInfo;
use crate::scanner::FileInfo;

use horrorshow::{helper::doctype, Raw, Template};

fn header<'a>(redirect_url: Option<&'a str>) -> Box<dyn horrorshow::RenderBox + 'a> {
    box_html! {
        title : "Eloran";
        meta(charset="UTF-8");
        meta(name="viewport", content="width=device-width");
        link(rel="stylesheet", href="/css/eloran.css");
        link(rel="stylesheet", href="/css/w3.css");
        link(rel="stylesheet", href="/css/w3-theme-dark-grey.css");
        meta(http-equiv="Cache-Control", content="no-cache, no-store, must-revalidate");
        meta(http-equiv="Pragma", content="no-cache");
        meta(http-equiv="Expires", content="0");
        // add a meta tag with url to redirect
        @ if let Some(url) = redirect_url {
            meta(http-equiv="refresh", content=format!("0; url='{url}'")) ;
        }
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
    current_page: i32,
    bookmark_status: bool,
    read_status: bool,
    up_link: String,
) -> String {
    let menu = menu(user.to_owned(), None);
    let file = file.clone();
    let body_content = box_html! {
        : menu;
        div(id="infos") {
            h2(style="text-align: center;") {
                a(href=format!("/read/{}/{}", file.id, current_page), class="navigation") : "📖 read file";
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
                    : format!("page : {}/{}", current_page, file.total_pages) ;
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
    let menu = menu(user.to_owned(), None);
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
    let menu = menu(user.to_owned(), None);
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

pub struct LibraryDisplay {
    pub user: User,
    pub directories_list: Vec<DirectoryInfo>,
    pub files_list: Vec<(FileInfo, bool, bool)>,
    pub library_path: String,
    pub current_path: Option<String>,
    pub search_query: Option<String>,
    // TODO need search query option string
}

pub fn library(list_to_display: LibraryDisplay) -> String {
    debug!("fn homepage");
    // we dispose of following variables :
    // - directory.name : Subdir2
    // - directory.parent_path : /home/thasos/mylibrary/Dragonlance
    // - current_path : /library/Dragonlance/Subdir1
    // - library_path : /home/thasos/mylibrary
    // we need (assume we are un Subdir1):
    // - the url path for up link : /library/Dragonlance
    // - current disk path : /home/thasos/mylibrary/Dragonlance/Subdir1
    // - directory url path : /library/Dragonlance/Subdir1/Subdir2

    // unless search, we need to construct an up_link
    let mut up_link = String::new();
    if let Some(current_path) = list_to_display.current_path.clone() {
        let mut full_path: Vec<&str> = current_path.split('/').collect();
        full_path.pop();
        // TODO better variable name
        for word in full_path {
            up_link.push_str(word);
            up_link.push('/');
        }
        up_link.pop();
    }

    // html rendering
    let menu = menu(
        list_to_display.user.to_owned(),
        list_to_display.search_query.to_owned(),
    );
    let body_content = box_html! {
        : menu;
        div(id="library-content") {
            // if we have a current_path, we can display some infos (unavailable in search)
            @ if let Some(current_path) = &list_to_display.current_path {
                p {
                    // TODO split and add a direct link for each element in path
                    : format!("list_to_display.library_path = {}, current_path = {}", list_to_display.library_path, &current_path);
                }
                h2 { a(href=format!("/library{}", &up_link), class="navigation") : "↖️  up" }
            }
            // image gallery
            // https://www.w3schools.com/Css/css_image_gallery.asp
            @ for directory in &list_to_display.directories_list.to_owned() {
                div(class="gallery box_shadow container") {
                    // remove disk parent path for url construction
                    a(href= {
                        // avoid double '/', I'm not proud of this...
                        if directory.parent_path.is_empty() {
                            format!("/library/{}", &directory.name)
                        } else {
                            format!("/library{}/{}", list_to_display.current_path.clone().unwrap_or("".to_string()), &directory.name)
                        }
                    }) {
                        div(class="cover") {
                            img(src="/images/folder.svgz", alt="folder", width="150", height="230");
                            @ if let Some(file_number) = directory.file_number{
                                div(class="file_number") {
                                    : file_number;
                                }
                            }
                        }
                        div(class="gallery_desc") {
                            : format_args!("{}", directory.name)
                        }
                    }
                }
            }
            @ for file in &list_to_display.files_list.to_owned() {
                div(class="gallery box_shadow container") {
                    a(href=format!("/infos/{}", &file.0.id)) {
                        div(class="cover") {
                            img(src=format!("/cover/{}", &file.0.id), alt="cover", width="150", height="230", class= if file.2 { "cover_read" } else { "cover" } );
                        }
                        div(class="gallery_desc") {
                            : format_args!("{}", file.0.name);
                        }
                    }
                    div(class="flags") {
                        : if file.1 { "⭐" } else { "" };
                        : if file.2 { "✅" } else { "" };
                    }
                }
            }
        }
    };
    render(body_content, None)
}

fn menu<'a>(user: User, search_query: Option<String>) -> Box<dyn horrorshow::RenderBox + 'a> {
    debug!("fn menu");
    // TODO print a pretty menu, 1 line...
    let menu_content = box_html! {
        div(id="menu") {
            p {
                a(href="/library") : "library" ;
                : " | ";
                a(href="/bookmarks") : "bookmarks" ;
                : " | ";
                a(href="/reading") : "reading" ;
                : " | ";
                a(href="/prefs") : "preferences" ;
                // print admin link if Role is ok
                @ if user.role == Role::Admin {
                    : " | ";
                    a(href="/admin") : "administration" ;
                }
                : " | ";
                : format!("{}", user.name.as_str()) ;
                : " (";
                a(href="/logout") : "logout" ;
                : ")";
            }
            form(action="/search", method="post") {
                @ if let Some(query) = &search_query {
                    input(type="text", placeholder=query, name="query", value=query) ;
                } else {
                    input(type="text", placeholder="Search..", name="query") ;
                }
            }
        }
    };
    menu_content
}

pub fn homepage(user: &User) -> String {
    debug!("fn homepage");
    // TODO moche (obligé le clone  ?)
    let menu = menu(user.to_owned(), None);
    let body_content = box_html! {
        : menu;
        div(id="home-content") {
        : "content"
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
            h2(id="heading") { : "Eloran" }
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
