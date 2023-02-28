use crate::http_server::User;
use crate::scanner::FileInfo;
use horrorshow::{helper::doctype, Template};

fn header<'a>() -> Box<dyn horrorshow::RenderBox + 'a> {
    // TODO css, metadatas...
    box_html! {
        title : "Eloran";
        meta(charset="UTF-8");
        meta(name="viewport", content="width=device-width");
        link(rel="stylesheet", href="css/w3.css");
        link(rel="stylesheet", href="css/gallery.css");
        link(rel="stylesheet", href="css/w3-theme-dark-grey.css");
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

pub fn library(user: &User, publication_list: Vec<FileInfo>) -> String {
    debug!("fn homepage");
    // TODO moche (obligé le clone  ?)
    let menu = menu(user.clone());
    let body_content = box_html! {
        : menu;
        div(id="library-content") {
            p {
                : "Library list";
            }
            // list naze
            // ul(id="publiations") {
            //     @ for publiation in &publication_list {
            //         li {
            //             : format_args!("{}/{}",publiation.parent_path ,publiation.filename)
            //         }
            //     }
            // }

            // table
            // https://www.w3schools.com/w3css/w3css_tables.asp
            // table(class="w3-table w3-centered w3-large") {
            //     tr {
            //         @ for publiation in &publication_list {
            //             th {
            //                 : format_args!("{}/{}",publiation.parent_path ,publiation.filename)
            //             }
            //         }
            //     }
            // }

            // image gallery
            // https://www.w3schools.com/Css/css_image_gallery.asp
            @ for publiation in &publication_list {
                div(class="gallery") {
                    img(src="https://www.w3schools.com/Css/img_5terre.jpg", alt="Cinque Terre", width="600", height="400")
                    : format_args!("{}", publiation.filename)
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
