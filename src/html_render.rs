use crate::http_server::{Role, User};
use crate::scanner::{DirectoryInfo, FileInfo, Library};

use horrorshow::{helper::doctype, Raw, Template};
use time::format_description;
use time::OffsetDateTime;

fn header<'a>(redirect_url: Option<&'a str>) -> Box<dyn horrorshow::RenderBox + 'a> {
    box_html! {
        title : "Eloran";
        meta(charset="UTF-8");
        meta(name="viewport", content="width=device-width");
        link(rel="stylesheet", href="/css/eloran.css");
        // favicon
        link(rel="icon", type="image/png", href="/favicon-96x96.png", sizes="96x96");
        link(rel="icon", type="image/svgz+xml", href="/favicon.svgz");
        link(rel="shortcut icon", href="/favicon.ico");
        link(rel="apple-touch-icon", sizes="180x180", href="/apple-touch-icon.png");
        meta(name="apple-mobile-web-app-title", content="Eloran");
        link(rel="manifest", href="/site.webmanifest");
        // cache control
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
    let menu = menu(None);
    let body_content = box_html! {
        : menu;
        div {
            p { : message; }
        }
    };
    render(body_content, origin)
}

pub fn prefs(user: &User) -> String {
    let menu = menu(Some(user.to_owned()));
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
    let menu = menu(Some(user.to_owned()));
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
        div(id="login", style="text-align: center;") {
            br;
            br;
            img(src="/images/library-icon.svgz");
            br;
            br;
            p { : "Please login" }
            br;
            br;
            p {
                form(accept-charset="utf-8", action="/login", method="post") {
                input(type="text", name="username", placeholder="username", required);
                br;
                br;
                input(type="password", name="password", placeholder="password", required);
                br;
                br;
                input(type="submit", value="Login");
                }
            }
        }
    };

    render(body_content, None)
}

pub fn logout() -> String {
    let body_content = box_html! { p
        // { : format!("Bye {}", user.name.as_str()) }
        { : format!("Bye !") }
        p { a(href="/") : "return home" }
    };

    let redirect_url = "/";
    render(body_content, Some(redirect_url))
}

fn timestamp_to_pretty_date(timestamp: i64) -> Option<String> {
    let pretty_date_format = format_description::parse("[year]-[month]-[day]").ok()?;
    let added_date = OffsetDateTime::from_unix_timestamp(timestamp).ok()?;
    let pretty_added_date = added_date.format(&pretty_date_format).ok()?;
    Some(pretty_added_date)
}

pub fn file_info(
    user: &User,
    file: &FileInfo,
    current_page: i32,
    bookmark_status: bool,
    read_status: bool,
    up_link: String,
) -> String {
    let menu = menu(Some(user.to_owned()));
    // we need to clone file infos, don't remember why...
    let file = file.clone();
    // format added date
    let pretty_added_date = match timestamp_to_pretty_date(file.added_date) {
        Some(pretty_added_date) => pretty_added_date,
        None => String::from("not available"),
    };
    // human readable file size without lib
    let pretty_file_size = if file.size < 1024 * 1024 {
        format!("{:.3} kB", file.size as f32 / 1024.00)
    } else if file.size >= 1024 && file.size < 1024 * 1024 * 1024 {
        format!("{} MB", file.size / 1024 / 1024)
    } else {
        format!("{:.1} GB", file.size as f64 / 1024.00 / 1024.00 / 1024.00)
    };
    // construct file library path for breadcrumb
    let mut breadcrumb_link_path = String::new();
    // separe path elements, delete absolute path...
    let file_library_path: Vec<&str> = file
        .parent_path
        .split('/')
        .skip_while(|s| *s != file.library_name)
        .collect();
    // ... and reassamble
    let file_library_path = file_library_path.join("/");
    // body
    let body_content = box_html! {
        : menu;
        main {
            header {
                a(href="/library") {
                    img(src="/images/library-icon.svgz") ;
                    h1 { : &file.name ; }
                }
            }
            section(class="filters") {
                ul(class="breadcrumb") {
                    li { a(href=format!("/library"), class="navigation") : "Library" }
                    div(class="border-arrow") { div(class="arrow") {} }
                        // first, split only last element
                        @ if let Some(rsplitted_current_path) = file_library_path.rsplit_once('/') {
                            // then loop on all directories
                            @ for sub_directory in rsplitted_current_path.0.split('/') {
                                li {
                                    a(href=format!(
                                        "/library/{}",
                                        { breadcrumb_link_path.push_str(&(sub_directory.to_owned() + "/")) ; &breadcrumb_link_path.trim_end_matches('/') }
                                    ))
                                    : sub_directory
                                }
                                div(class="border-arrow") { div(class="arrow") {} }
                            }
                            // last element of breadcrumb must be css class `selected`
                            // and no arrow
                            li(class="selected") { a(href=format!("/library/{}/{}", rsplitted_current_path.0, &rsplitted_current_path.1)) : rsplitted_current_path.1 }
                        } else {
                            // file is at the lib root
                            li(class="selected") { a(href=format!("/library/{}", file_library_path)) : file_library_path }
                        }

                }
                // TODO put search elsewhere in code ?
                div(class="search") {
                    form(accept-charset="utf-8", action="/search", method="post") {
                        input(type="submit", value="");
                        input(type="text", placeholder="Search...", name="query", value="");
                    }
                }
            }

            // file infos
            div(id="infos", style="text-align: center;") {
                br;
                br;
                br;
                h2 {
                    a(href= up_link , class="navigation") : "↖️  up";
                    : " | " ;
                    @ if file.format != "pdf" {
                        a(href=format!("/read/{}/{}", file.id, current_page), class="navigation") : "📖 read file";
                        : " | " ;
                    }
                    a(href=format!("/download/{}", file.id), class="navigation") : "⤵ download";
                    : " | " ;
                    a(href=format!("/toggle/bookmark/{}", file.id)) : if bookmark_status { "⭐ (remove from bookmarks)" } else { "(bookmark)" } ;
                    : " | " ;
                    a(href=format!("/toggle/read_status/{}", file.id)) : if read_status { "✅ (mark as unread)" } else { "(mark as read)" };
                }
                br;
                br;
                @ if file.format != "pdf" {
                    a(href=format!("/read/{}/{}", file.id, current_page), class="navigation") {
                        img(src=format!("/cover/{}", file.id), alt="cover", class="infos");
                    }
                } else {
                    a(href=format!("/download/{}", file.id), class="navigation") {
                        img(src=format!("/cover/{}", file.id), alt="cover", class="infos");
                    }
                }
                br;
                br;
                p(style="text-align: center;") {
                    : file.name ;
                    br;
                    br;
                    : format!("size : {}", pretty_file_size) ;
                    br;
                    : format!("pages : {}/{}", current_page, file.total_pages) ;
                    br;
                    : format!("type : {}", file.format) ;
                    br;
                    : format!("added : {}", pretty_added_date) ;
                }
            }
        }
    };
    render(body_content, None)
}

pub fn flag_toggle(user: &User, flag_status: bool, file_id: &str, flag: &str) -> String {
    let menu = menu(Some(user.to_owned()));
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
    let menu = menu(Some(user.to_owned()));
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
            a(href=format!("/read/{}/{}", file.id, previous_page), class="navigation") : "⏪";
            : " | " ;
            a(href=format!("/read/{}/{}", file.id, 0), class="navigation") : "⏮ start";
            : " | " ;
            a(href=format!("/infos/{}", file.id), class="navigation") : "return to file info";
            : " | " ;
            a(href=format!("/read/{}/{}", file.id, file.total_pages - 1), class="navigation") : "end ⏭";
            : " | " ;
            a(href=format!("/read/{}/{}", file.id, next_page), class="navigation") : "⏩";
        }
        br;
        br;
        div(class="reader-full-page-image") {
            picture {
                source(srcset=format!("/comic_page/{}/{}/800px", file.id, page), media="(max-width: 800px)", class="comic-content");
                source(srcset=format!("/comic_page/{}/{}/1000px", file.id, page), media="(max-width: 1000px)", class="comic-content");
                source(srcset=format!("/comic_page/{}/{}/orig", file.id, page), class="comic-content");
                img(src=format!("/comic_page/{}/{}/orig", file.id, page), alt="TODO_PAGE_NUM", class="comic-content", usemap="reader-full-page-image");
                // not a html map, because we need percentage coords
                // thx https://stackoverflow.com/a/26231487
                a(href="", style="top: 0%; left: 30%; width: 40%; height: 3%;"); // zone for menu
                a(href=format!("/read/{}/{}", file.id, previous_page), style="top: 0%; left: 0%; width: 30%; height: 100%;");
                a(href=format!("/read/{}/{}", file.id, next_page), style="top: 0%; left: 70%; width: 30%; height: 100%;");
            }
        }
    };
    render(body_content, None)
}

pub fn ebook_reader(user: &User, file: &FileInfo, epub_content: &str, page: i32) -> String {
    let menu = menu(Some(user.to_owned()));
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

    // fill search button value if needed
    let search_value = list_to_display.search_query.to_owned().unwrap_or_default();

    // String used to build breadcrumb links
    let mut breadcrumb_link_path = String::new();

    // html rendering
    let menu = menu(Some(list_to_display.user.to_owned()));
    let body_content = box_html! {
        : menu;
        main {
            header {
                a(href="/library") {
                    img(src="/images/library-icon.svgz") ;
                    h1 { : "Library" }
                }
            }
            section(class="filters") {
                // TODO do not print this in case on search
                ul(class="breadcrumb") {
                    li { a(href=format!("/library"), class="navigation") : "Library" }
                    @ if let Some(current_path) = &list_to_display.current_path {
                        // first, split only last element
                        @ if let Some(rsplitted_current_path) = current_path.rsplit_once('/') {
                            // then loop on all directories
                            @ for sub_directory in rsplitted_current_path.0.split('/') {
                                li {
                                    a(href=format!(
                                        "/library{}",
                                        { breadcrumb_link_path.push_str(&(sub_directory.to_owned() + "/")) ; &breadcrumb_link_path.trim_end_matches('/') }
                                    ))
                                    : sub_directory
                                }
                                div(class="border-arrow") { div(class="arrow") {} }
                            }
                            // last element of breadcrumb must be css class `selected`
                            // and no arrow
                            li(class="selected") { a(href=format!("/library{}/{}", rsplitted_current_path.0, &rsplitted_current_path.1)) : rsplitted_current_path.1 }
                        }
                    }
                }
                // TODO put search elsewhere in code ?
                div(class="search") {
                    form(accept-charset="utf-8", action="/search", method="post") {
                        input(type="submit", value="");
                        input(type="text", placeholder="Search...", name="query", value=search_value);
                    }
                }
            }

            // TODO not visible : use new CSS
            // if lists are empty, print a message
            @ if list_to_display.directories_list.is_empty() && list_to_display.files_list.is_empty() && &list_to_display.library_path == "/" {
                p {
                    br;
                    br;
                    br;
                    a(href="/admin") : "Please add a library in Administration panel"
                }
            } else if list_to_display.directories_list.is_empty() && list_to_display.files_list.is_empty() {
                p {
                    // TODO need library name or id here (in struct LibraryDisplay)
                    // TODO do not print in case of search...
                    : format!("Library {} is empty, please be patient", &list_to_display.library_path);
                    // TODO remove this ugly unwrap
                    form(action=format!("/admin/library/{}", &list_to_display.library_id.unwrap_or(0)), method="post") {
                        div {
                            input(type="submit", name="full_rescan", value="Force a full rescan");
                        }
                    }
                }
            }

            section(class="gallery") {
                @ for directory in &list_to_display.directories_list.to_owned() {
                    article(class="folder") {
                        a(href= {
                            // avoid double '/', I'm not proud of this...
                            if directory.parent_path.is_empty() {
                                format!("/library/{}", &directory.name)
                            } else {
                                format!("/library{}/{}", list_to_display.current_path.clone().unwrap_or("".to_string()), &directory.name)
                            }
                        }) {
                            div(class="cover") {
                                span(class="folder-img") {}
                                @ if let Some(file_count) = directory.file_count {
                                    span(class="folder-nb-items")
                                        : file_count;
                                }
                            }
                            div(class="title") { h2 { : format_args!("{}", directory.name) } }
                        }
                        // TODO add toggle bookmark
                        // @ if file.1 {
                        //     button(class="favorite bookmarked")
                        // } else {
                        //     button(class="favorite")
                        // }
                    }
                }
                @ for file in &list_to_display.files_list.to_owned() {
                    article(class="file") {
                        a(href=format!("/infos/{}", &file.0.id)) {
                            div(class="cover") {
                                // blurred cover
                                img(src=format!("/cover/{}", &file.0.id), alt="blurred background cover", class= if file.2 {
                                    "blurred-background read"
                                } else {
                                    "blurred-background"
                                } );
                                // resized cover, with read status
                                img(src=format!("/cover/{}", &file.0.id), alt="cover", class= if file.2 {
                                    "cover read"
                                } else {
                                    "cover"
                                } );
                            }
                        }
                        div(class="title") { h2 { : format_args!("{}", file.0.name); } }
                        // add toggle link
                        @ if file.1 {
                            a(href=format!("/toggle/bookmark/{}", file.0.id)) {
                                button(class="favorite bookmarked")
                            }
                        } else {
                            a(href=format!("/toggle/bookmark/{}", file.0.id)) {
                                button(class="favorite")
                            }
                        }
                    }
                }
            }
        }
    };
    render(body_content, None)
}

fn menu<'a>(user: Option<User>) -> Box<dyn horrorshow::RenderBox + 'a> {
    // TODO print a pretty menu, 1 line...
    let menu_content = box_html! {
        header {
            div(class="logo") {
                a(href="/library") { : "Eloran" }
            }
            nav {
                input(type="checkbox", id="lasagna-checkbox");
                button(class="rounded-button lasagna-button") {
                    span(class="selected-rounded-button") {}
                    label(for="lasagna-checkbox") {
                        img(src="/images/lasagna.svgz");
                    }
                }
                ul(class="menu") {
                    li { a(href="/library", class="nav-button nav-button-1") : "Library" ; }
                    li { a(href="/reading", class="nav-button nav-button-2") : "Reading" ; }
                    li { a(href="/bookmarks", class="nav-button nav-button-3") : "Bookmarks" ; }
                    input(type="checkbox", id="prefs-checkbox");
                    button(class="rounded-button prefs-button") {
                        span(class="selected-rounded-button") {}
                        label(for="prefs-checkbox") { : "A" ; }
                    }
                    ul(class="prefs-menu") {
                        li { a(href="/prefs") : "Preferences" ; }
                        // print admin link if Role is ok
                        @ if let Some(user) = user {
                            @ if user.role == Role::Admin {
                                li {
                                a(href="/admin") : "Administration" ;
                                }
                            }
                            li { a(href="/logout") : "Logout" ; }
                        }
                    }
                }
            }
        }
    };
    menu_content
}

/// take body content html box, and return all the page with headers and full body
fn render(body_content: Box<dyn horrorshow::RenderBox>, redirect_url: Option<&str>) -> String {
    let full_page = html! { : doctype::HTML;
    html {
        head { : header(redirect_url); }
            body(class="page-library") {
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
    fn test_logout() {
        insta::assert_yaml_snapshot!(logout())
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
        let menu = menu(Some(user));
        let rendered_menu = render(menu, Some(redirect_url));
        insta::assert_yaml_snapshot!(rendered_menu)
    }
}
