use horrorshow::helper::doctype;

pub enum Page {
    Login,
    Logout,
    Root,
    // the "Should not happen" page...
    // see https://baldursgate.fandom.com/wiki/Biff_the_Understudy
    BiffTheUnderstudy,
}

fn header<'a>() -> Box<dyn horrorshow::RenderBox + 'a> {
    // TODO css, metadatas...
    box_html! { title : "Eloran" }
}

fn login_form<'a>() -> Box<dyn horrorshow::RenderBox + 'a> {
    box_html! {
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
    }
}

fn logout<'a>(username: &'a str) -> Box<dyn horrorshow::RenderBox + 'a> {
    box_html! { p { : format!("Bye {username}") } }
}

fn root<'a>(username: &'a str) -> Box<dyn horrorshow::RenderBox + 'a> {
    // TODO add role ?
    box_html! { p { : format!("Logged in as: {username}") } }
}

// TODO find prettier way than a tuple
pub fn render((page, username): (Page, Option<String>)) -> String {
    let username = match username {
        Some(username) => username,
        // TODO should not happen, better way ?
        None => "unknow_user".to_string(),
    };
    format!(
        "{}",
        html! { : doctype::HTML;
            html {
                head { : header(); }
                body {
                    h2(id="heading") { : "Welcome to Eloran" }
                    : match page {
                        Page::Login => login_form(),
                        Page::Root => root(username.as_str()),
                        Page::Logout => logout(username.as_str()),
                        // TODO an basic error page ?
                        _ => todo!(),
                    }
                }
            }
        }
    )
}
