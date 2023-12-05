use crate::http_server::{Role, User};
use crate::scanner::{DirectoryInfo, FileInfo, Library};

use horrorshow::{helper::doctype, Raw, Template};

fn header<'a>(redirect_url: Option<&'a str>) -> Box<dyn horrorshow::RenderBox + 'a> {
    box_html! {
        title : "Eloran";
        meta(charset="UTF-8");
        meta(name="viewport", content="width=device-width");
        link(rel="stylesheet", href="/css/eloran.css");
        link(rel="stylesheet", href="/css/w3.css");
        meta(http-equiv="Cache-Control", content="no-cache, no-store, must-revalidate");
        meta(http-equiv="Pragma", content="no-cache");
        meta(http-equiv="Expires", content="0");
        // add a meta tag with url to redirect
        @ if let Some(url) = redirect_url {
            meta(http-equiv="refresh", content=format!("0; url='{url}'")) ;
        }
    }
}

pub fn simple_message(message: &str, origin: Option<&str>) -> String {
    let message = message.to_owned();
    let origin = origin.to_owned();
    let menu = menu(None, None);
    let body_content = box_html! {
        : menu;
        div {
            p { : message; }
        }
    };
    render(body_content, origin)
}

pub fn prefs(user: &User) -> String {
    let menu = menu(Some(user.to_owned()), None);
    let body_content = box_html! {
        : menu;
        h2 { : "Preferences" }
        div {
            p { : "(todo) change password"; }
            p { : "(todo) display all files or just readables"; }
            p { : "(todo) grid or list view"; }
            p { : "(todo) theme : dark or light"; }
        }
    };
    render(body_content, None)
}

pub fn admin(user: &User, library_list: Vec<Library>, user_list: Vec<User>) -> String {
    let menu = menu(Some(user.to_owned()), None);
    let body_content = box_html! {
        : menu;
        h2 { : "Admin Panel" }
        h3 { : "Libraries Path" }
        div {
            ul {
                @ for library in library_list {
                    li(class="item") {
                        form(action=format!("/admin/library/{}", library.id), method="post") {
                            div {
                                : library.name;
                                : " ";
                                input(type="submit", name="delete", value="Delete");
                                : " ";
                                input(type="submit", name="full_rescan", value="Full Rescan");
                                : " ";
                                input(type="submit", name="covers", value="Disable Covers (todo)");
                            }
                        }
                    }
                }
                li {
                    form(accept-charset="utf-8", action="/admin/library/new", method="post") {
                        input(type="text", name="path", placeholder="absolute path", required);
                        input(type="submit", value="New library path");
                    }
                }
            }
        }
        h3 { : "Options" }
        div {
            ul {
                li {
                    : "periodic library scan sleep time";
                    form(accept-charset="utf-8", action="/scan_sleep_time", method="post") {
                        input(type="text", name="scan_period", placeholder="in seconds", required);
                        input(type="submit", value="Update (todo)");
                    }
                }
                li {
                    : "periodic covers extraction sleep time";
                    form(accept-charset="utf-8", action="/extract_sleep_time", method="post") {
                        input(type="text", name="extract_periode", placeholder="in seconds", required);
                        input(type="submit", value="Update (todo)");
                    }
                }
            }
        }
        h3 { : "Users" }
        div {
            ul {
                @ for user in user_list {
                    li(class="item") {
                        div {
                            form(accept-charset="utf-8", action=format!("/admin/user/{}", &user.id), method="post") {
                                label { : &user.name }
                                : " ";
                                input(type="password", name="password", placeholder="password");
                                : " ";
                                @ if user.role == Role::Admin {
                                    input(type="checkbox", id="admin_box", name="is_admin", checked)
                                } else {
                                    input(type="checkbox", id="admin_box", name="is_admin")
                                }
                                label(for="admin_box") { : " Admin " }
                                input(type="submit", name="update", value="Update");
                                : " ";
                                input(type="submit", name="delete", value="Delete");
                            }
                        }
                    }
                }
                li {
                    form(accept-charset="utf-8", action="/admin/user/new", method="post") {
                        input(type="text", name="name", placeholder="name", required);
                        : " ";
                        input(type="password", name="password", placeholder="password", required);
                        : " ";
                        input(type="checkbox", id="admin_box", name="is_admin");
                        label(for="admin_box") { : " Admin " }
                        input(type="submit", value="New user");
                    }
                }
            }
        }
        h3 { : "Stats" }
        div {
            ul {
                li { : "Number of publication : 🤷" }
                li { : "Number of users : 🤷" }
                li { : "Publication readed : 🤷" }
                li { : "Publication bookmarked : 🤷" }
            }
        }
    };

    render(body_content, None)
}

pub fn login_form() -> String {
    let body_content = box_html! {
        p { : "Please login :" }
        p {
            form(accept-charset="utf-8", action="/login", method="post") {
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
    let menu = menu(Some(user.to_owned()), None);
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
    let menu = menu(Some(user.to_owned()), None);
    // TODO create enum for flag...
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

pub fn comic_reader(user: &User, file: &FileInfo, page: i32) -> String {
    let menu = menu(Some(user.to_owned()), None);
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
        div(id="comic-content") {
            picture {
                source(srcset=format!("/comic_page/{}/{}/800px", file.id, page), media="(max-width: 800px)");
                source(srcset=format!("/comic_page/{}/{}/1000px", file.id, page), media="(max-width: 1000px)");
                source(srcset=format!("/comic_page/{}/{}/orig", file.id, page));
                img(src=format!("/comic_page/{}/{}/orig", file.id, page), alt="TODO_PAGE_NUM");
            }
        }
    };
    render(body_content, None)
}

pub fn ebook_reader(user: &User, file: &FileInfo, epub_content: &str, page: i32) -> String {
    let menu = menu(Some(user.to_owned()), None);
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
    pub library_id: Option<i64>, // not really need this, see full_rescan button when lib is empty
    pub library_path: String,
    pub current_path: Option<String>,
    pub search_query: Option<String>,
    // TODO need search query option string
}

pub fn library_display(list_to_display: LibraryDisplay) -> String {
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
        for element in full_path {
            up_link.push_str(element);
            up_link.push('/');
        }
        up_link.pop();
    }

    // html rendering
    let menu = menu(
        Some(list_to_display.user.to_owned()),
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

            // if lists are empty, print a message
            @ if list_to_display.directories_list.is_empty() && list_to_display.files_list.is_empty() && &list_to_display.library_path == "/" {
                p {
                    : format!("Please add a library in ");
                    a(href="/admin") : "admin panel"
                }
            } else if list_to_display.directories_list.is_empty() && list_to_display.files_list.is_empty() {
                p {
                    // TODO need library name or id here (in struct LibraryDisplay)
                    : format!("Library {} is empty, please be patient", &list_to_display.library_path);
                    // TODO remove this ugly unwrap
                    form(action=format!("/admin/library/{}", &list_to_display.library_id.unwrap_or(0)), method="post") {
                        div {
                            input(type="submit", name="full_rescan", value="Force a full rescan");
                        }
                    }
                }
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

fn menu<'a>(
    user: Option<User>,
    search_query: Option<String>,
) -> Box<dyn horrorshow::RenderBox + 'a> {
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
                @ if let Some(user) = user {
                    @ if user.role == Role::Admin {
                        : " | ";
                        a(href="/admin") : "administration" ;
                    }
                    : " | ";
                    : format!("{} - {:?}", user.name.as_str(), user.role) ;
                    : " (";
                    a(href="/logout") : "logout" ;
                    : ")";
                }
            }
            form(accept-charset="utf-8", action="/search", method="post") {
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
    // TODO moche (obligé le clone  ?)
    let menu = menu(Some(user.to_owned()), None);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headers() {
        let redirect_url = "tests";
        // TODO WTF ?
        let rendered_headers = render(header(Some(&redirect_url)), Some(&redirect_url));
        insta::assert_yaml_snapshot!(rendered_headers)
    }
    #[test]
    fn test_simple_message() {
        insta::assert_yaml_snapshot!(simple_message("simple", Some("test")))
    }
    #[test]
    fn test_prefs() {
        let user = User::default();
        insta::assert_yaml_snapshot!(prefs(&user))
    }
    #[test]
    fn test_admin() {
        let user = User::default();
        let library_list = Vec::with_capacity(0);
        let user_list = Vec::with_capacity(0);
        insta::assert_yaml_snapshot!(admin(&user, library_list, user_list))
    }
    #[test]
    fn test_login_form() {
        insta::assert_yaml_snapshot!(login_form())
    }
    #[test]
    fn test_login_ok() {
        let user = User::default();
        insta::assert_yaml_snapshot!(login_ok(&user))
    }
    #[test]
    fn test_logout() {
        let user = User::default();
        insta::assert_yaml_snapshot!(logout(&user))
    }
    #[test]
    fn test_file_info() {
        let user = User::default();
        let file = FileInfo::default();
        let current_page: i32 = 2;
        let bookmark_status = false;
        let read_status = true;
        let up_link = String::from("some/up/link");
        insta::assert_yaml_snapshot!(file_info(
            &user,
            &file,
            current_page,
            bookmark_status,
            read_status,
            up_link
        ))
    }
    #[test]
    fn test_flag_toggle() {
        let user = User::default();
        let flag_status = true;
        let file_id = "blabla";
        let flag = "read_status";
        insta::assert_yaml_snapshot!(flag_toggle(&user, flag_status, file_id, flag))
    }
    #[test]
    fn test_comic_reader() {
        let user = User::default();
        let file = FileInfo::default();
        let page: i32 = 10;
        insta::assert_yaml_snapshot!(comic_reader(&user, &file, page))
    }
    #[test]
    fn test_ebook_reader() {
        let user = User::default();
        let file = FileInfo::default();
        let epub_content = "Lorem ipsum dolor sit amet";
        let page: i32 = 10;
        insta::assert_yaml_snapshot!(ebook_reader(&user, &file, epub_content, page))
    }
    #[test]
    fn test_library() {
        let list_to_display = LibraryDisplay {
            user: User::default(),
            directories_list: Vec::with_capacity(0),
            files_list: Vec::with_capacity(0),
            library_id: None,
            library_path: String::from("some/path"),
            current_path: None,
            search_query: None,
        };
        insta::assert_yaml_snapshot!(library_display(list_to_display))
    }
    #[test]
    fn test_menu() {
        let user = User::default();
        let search_query = String::from("searching");
        let redirect_url = "tests";
        let menu = menu(Some(user), Some(search_query));
        let rendered_menu = render(menu, Some(redirect_url));
        insta::assert_yaml_snapshot!(rendered_menu)
    }
    #[test]
    fn test_homepage() {
        let user = User::default();
        insta::assert_yaml_snapshot!(homepage(&user))
    }
}
