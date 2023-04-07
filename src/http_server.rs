use crate::html_render::{self, login_ok};
use crate::reader;
use crate::scanner::{self, DirectoryInfo, FileInfo};
use crate::sqlite;

use axum::http::header;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::{
    extract::Path,
    routing::{get, post},
    Extension, Router,
};
use axum_login::{
    axum_sessions::{async_session::MemoryStore, SessionLayer},
    secrecy::SecretVec,
    AuthLayer, AuthUser, RequireAuthorizationLayer, SqliteStore,
};
use rand::Rng;
use std::fs;
use std::io::Error;
use tower::ServiceBuilder;

// User Struct
// TODO virer Default ?
#[derive(Debug, Default, Clone, sqlx::FromRow)]
pub struct User {
    // TODO type ulid ? KO with sqlite query_as
    pub id: i64,
    pub password_hash: String,
    pub name: String,
    pub role: Role,
}
impl AuthUser<Role> for User {
    fn get_id(&self) -> String {
        format!("{}", self.id)
    }
    fn get_password_hash(&self) -> SecretVec<u8> {
        SecretVec::new(self.password_hash.clone().into())
    }
    fn get_role(&self) -> Option<Role> {
        Some(Role::User)
    }
}

type AuthContext = axum_login::extractors::AuthContext<User, SqliteStore<User, Role>, Role>;

/// Roles
#[derive(Debug, Clone, PartialEq, PartialOrd, sqlx::Type, Default)]
pub enum Role {
    #[default]
    User,
    Admin,
}

fn parse_credentials(body: &str) -> (String, String) {
    let parsed_body: Vec<&str> = body.split('&').collect();
    let mut username = String::new();
    let mut password = String::new();
    for field in parsed_body {
        if let Some(usr) = field.strip_prefix("user=") {
            username = usr.to_string()
        }
        if let Some(pwd) = field.strip_prefix("password=") {
            password = pwd.to_string()
        }
    }
    (username, password)
}

async fn bookmarks_handler(Extension(user): Extension<User>) -> impl IntoResponse {
    info!("get /bookmarks : {}", &user.name);
    let conn = sqlite::create_sqlite_pool().await;
    // search files
    let mut files_results = sqlite::bookmarks_for_user_id(user.id, &conn).await;
    files_results.sort();
    // add status (read, bookmark)
    let user_id = sqlite::get_user_id_from_name(&user.name, &conn).await;
    let mut files_results_with_status: Vec<(FileInfo, bool, bool)> = Vec::default();
    for file in files_results {
        let bookmark_status = sqlite::get_flag_status("bookmark", user_id, &file.id, &conn).await;
        let read_status = sqlite::get_flag_status("read_status", user_id, &file.id, &conn).await;
        files_results_with_status.push((file, bookmark_status, read_status));
    }
    // lib path
    let library_path: String = sqlite::get_library_path(&conn).await;
    conn.close().await;
    // response
    let list_to_display = html_render::LibraryDisplay {
        user: user.clone(),
        directories_list: Vec::default(),
        files_list: files_results_with_status,
        library_path,
        current_path: None,
        search_query: None,
    };
    Html(html_render::library(list_to_display))
}

async fn search_handler(Extension(user): Extension<User>, query: String) -> impl IntoResponse {
    info!("get /search : {}", &query);
    // body string is `query=search_string`, we need only the `search_string`
    let query = query.strip_prefix("query=").unwrap();
    let query = &query.replace('+', " ");
    let conn = sqlite::create_sqlite_pool().await;
    // search files
    let mut files_results = sqlite::search_file_from_string(query, &conn).await;
    files_results.sort();
    // add status (read, bookmark)
    let user_id = sqlite::get_user_id_from_name(&user.name, &conn).await;
    let mut files_results_with_status: Vec<(FileInfo, bool, bool)> = Vec::default();
    for file in files_results {
        let bookmark_status = sqlite::get_flag_status("bookmark", user_id, &file.id, &conn).await;
        let read_status = sqlite::get_flag_status("read_status", user_id, &file.id, &conn).await;
        files_results_with_status.push((file, bookmark_status, read_status));
    }
    // search dirs
    let mut directories_results = sqlite::search_directory_from_string(query, &conn).await;
    directories_results.sort();
    // lib path
    let library_path: String = sqlite::get_library_path(&conn).await;
    conn.close().await;
    // response
    let list_to_display = html_render::LibraryDisplay {
        user: user.clone(),
        directories_list: directories_results,
        files_list: files_results_with_status,
        library_path,
        current_path: None,
        search_query: Some(query.to_string()),
    };
    Html(html_render::library(list_to_display))
}

async fn login_handler(mut auth: AuthContext, body: String) -> impl IntoResponse {
    info!("get /login : {}", &body);
    let (username, password) = parse_credentials(&body);
    let conn = sqlite::create_sqlite_pool().await;
    // get user from db
    // TODO hash password
    Html({
        let login_response =
            match sqlx::query_as("SELECT * FROM users WHERE name = ? AND password_hash = ?;")
                .bind(&username)
                .bind(&password)
                .fetch_one(&conn)
                .await
            {
                Ok(user) => {
                    // TODO check if password match
                    auth.login(&user).await.unwrap();
                    login_ok(&user)
                }
                Err(_) => {
                    warn!("user {} not found", &username);
                    // TODO : vraie page
                    "user not found".to_string()
                }
            };
        conn.close().await;
        login_response
    })
}

async fn logout_handler(mut auth: AuthContext) -> impl IntoResponse {
    info!("get /logout : {:?}", &auth.current_user);
    auth.logout().await;
    match &auth.current_user {
        Some(user) => {
            debug!("user found, logout");
            Html(html_render::logout(user))
        }
        None => {
            warn!("no user found, can't logout !");
            Html("Err".to_string())
        }
    }
}

// #[axum::debug_handler]
// TODO link "previous page" or folder of publication
async fn infos_handler(
    Extension(user): Extension<User>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let conn = sqlite::create_sqlite_pool().await;
    let file = sqlite::get_files_from_id(&id, &conn).await;
    // path for up link
    let library_path = sqlite::get_library_path(&conn).await;
    let up_link = file.parent_path.replacen(&library_path, "/library", 1);
    // total_page = 0, we need to scan it
    if file.scan_me == 1 {
        scanner::extract_all(&file, &conn).await;
    }
    // we need user_id for bookmark and read status
    let user_id = sqlite::get_user_id_from_name(&user.name, &conn).await;
    let bookmark_status = sqlite::get_flag_status("bookmark", user_id, &file.id, &conn).await;
    let read_status = sqlite::get_flag_status("read_status", user_id, &file.id, &conn).await;
    conn.close().await;
    Html(html_render::file_info(
        &user,
        &file,
        bookmark_status,
        read_status,
        up_link,
    ))
}

/// add/remove flag (bookmark or read status) of a file for a user
async fn flag_handler(
    Extension(user): Extension<User>,
    Path((flag, file_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let conn = sqlite::create_sqlite_pool().await;
    let user_id = sqlite::get_user_id_from_name(&user.name, &conn).await;
    let flag_status = sqlite::set_flag_status(&flag, user_id, &file_id, &conn).await;
    conn.close().await;
    Html(html_render::flag_toggle(
        &user,
        flag_status,
        &file_id,
        &flag,
    ))
}

async fn cover_handler(
    Extension(_user): Extension<User>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let conn = sqlite::create_sqlite_pool().await;
    let file = sqlite::get_files_from_id(&id, &conn).await;
    debug!("get /cover/{}", id);
    // defaut cover definition
    let default_cover = {
        let image_file_content = fs::read("src/images/green_book.svgz");
        match image_file_content {
            Ok(image) => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "image/svg+xml")],
                [(header::CONTENT_ENCODING, "gzip")],
                [(header::VARY, "Accept-Encoding")],
                image,
            )
                .into_response(),
            Err(_) => {
                error!("default cover /images/green_book.svgz not found");
                // TODO true 404 page
                (StatusCode::NOT_FOUND, "image not found").into_response()
            }
        }
    };
    // get cover from database
    // return default cover if problem with database or cover empty
    let u8_cover = sqlite::get_cover_from_id(&file, &conn).await;
    conn.close().await;
    match u8_cover {
        Some(cover) => {
            if !cover.is_empty() {
                (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "image/jpeg")],
                    cover,
                )
                    .into_response()
            } else {
                default_cover
            }
        }
        None => default_cover,
    }
}

// TODO filename...
async fn download_handler(
    Extension(user): Extension<User>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let conn = sqlite::create_sqlite_pool().await;
    info!("get /download/{} : {}", &id, &user.name);
    let file = sqlite::get_files_from_id(&id, &conn).await;
    let full_path = format!("{}/{}", file.parent_path, file.name);
    dbg!(&full_path);
    // Html(full_path).into_response()
    // possible content-types : https://www.iana.org/assignments/media-types/media-types.xhtml
    let content_type = match file.format.as_str() {
        "epub" => "application/epub+zip",
        "pdf" => "application/pdf",
        "cbz" => "application/vnd.comicbook+zip",
        "cbr" => "application/vnd.comicbook-rar",
        _ => "",
    };
    if let Ok(file_content) = fs::read(&full_path) {
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, content_type)],
            file_content,
        )
            .into_response()
    } else {
        (StatusCode::NOT_FOUND, "file not found").into_response()
    }
}

async fn reader_handler(
    Extension(user): Extension<User>,
    Path((id, page)): Path<(String, i32)>,
) -> impl IntoResponse {
    // TODO set current page to 0 if not provided ?
    // let page: i32 = page.unwrap_or(0);
    let conn = sqlite::create_sqlite_pool().await;
    info!("get /reader/{} (page {}) : {}", &id, &page, &user.name);
    let file = sqlite::get_files_from_id(&id, &conn).await;
    // total_page = 0, we need to scan it
    if file.scan_me == 1 {
        scanner::extract_all(&file, &conn).await;
    }
    // don't go outside the files
    let page = if page > file.total_pages - 1 {
        file.total_pages - 1
    } else {
        page
    };
    // mark as read if last page
    if page == file.total_pages - 1
        && !sqlite::get_flag_status("read_status", user.id as i32, &file.id, &conn).await
    {
        let _ = sqlite::set_flag_status("read_status", user.id as i32, &file.id, &conn).await;
    }
    // set page at current_page
    sqlite::set_current_page_from_id(&file.id, &page, &conn).await;

    let response = match file.format.as_str() {
        "epub" => {
            let epub_reader = reader::epub(&file, page).await;
            Html(html_render::ebook_reader(&user, &file, &epub_reader, page)).into_response()
        }
        "pdf" => {
            let pdf_file = fs::read(format!("{}/{}", &file.parent_path, &file.name));
            match pdf_file {
                Ok(pdf_file) => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "application/pdf")],
                    pdf_file,
                )
                    .into_response(),
                Err(e) => {
                    error!(
                        "pdf file {}/{} not found : {e}",
                        &file.parent_path, &file.name
                    );
                    // TODO true 404
                    (StatusCode::NOT_FOUND, "file not found").into_response()
                }
            }
        }
        // "cbr" => reader::cbr(&user, file),
        "cbz" | "cbr" | "cb7" => {
            let comic_reader = reader::comics(&file, page).await;
            Html(html_render::ebook_reader(&user, &file, &comic_reader, page)).into_response()
        }
        // TODO txt and raw readers
        // "txt" => reader::txt(&user, file),
        // "raw" => reader::raw(&user, file),
        // TODO real rendered page
        _ => Html("no yet supported".to_string()).into_response(),
    };
    conn.close().await;
    response
}

async fn library_handler(
    Extension(user): Extension<User>,
    path: Option<Path<String>>,
) -> impl IntoResponse {
    let conn = sqlite::create_sqlite_pool().await;
    let library_path: String = sqlite::get_library_path(&conn).await;
    let path = match path {
        Some(path) => format!("/{}", path.as_str()),
        None => String::new(),
    };

    // we need user_id for bookmark and read status
    let user_id = sqlite::get_user_id_from_name(&user.name, &conn).await;

    // TODO add bookmark and read status with get_bookmark_status()
    let mut files_list_with_status: Vec<(FileInfo, bool, bool)> = {
        info!("get /library{} : {}", path, user.name);
        // TODO set limit in conf
        let files_list: Vec<FileInfo> = match sqlx::query_as(&format!(
            "SELECT * FROM files WHERE parent_path = '{}{}' ORDER BY name",
            library_path,
            path.replace('\'', "''")
        ))
        .fetch_all(&conn)
        .await
        {
            Ok(files_list) => files_list,
            Err(e) => {
                warn!("empty library : {}", e);
                let empty_list: Vec<FileInfo> = Vec::new();
                empty_list
            }
        };
        // add bookmark and read status to the list
        let mut files_list_with_status: Vec<(FileInfo, bool, bool)> = Vec::default();
        for file in files_list {
            let bookmark_status =
                sqlite::get_flag_status("bookmark", user_id, &file.id, &conn).await;
            let read_status =
                sqlite::get_flag_status("read_status", user_id, &file.id, &conn).await;
            files_list_with_status.push((file, bookmark_status, read_status));
        }
        files_list_with_status
    };
    files_list_with_status.sort();

    let mut directories_list: Vec<DirectoryInfo> = {
        info!("get /library{} : {}", path, user.name);
        // TODO set limit in conf
        let directories_list: Vec<DirectoryInfo> = match sqlx::query_as(&format!(
            "SELECT * FROM directories WHERE parent_path = '{}{}'",
            library_path,
            path.replace('\'', "''")
        ))
        .fetch_all(&conn)
        .await
        {
            Ok(directories_list) => directories_list,
            Err(e) => {
                warn!("empty library : {}", e);
                let empty_list: Vec<DirectoryInfo> = Vec::new();
                empty_list
            }
        };
        directories_list
    };
    directories_list.sort();
    conn.close().await;
    let list_to_display = html_render::LibraryDisplay {
        user: user.clone(),
        directories_list,
        files_list: files_list_with_status,
        library_path,
        current_path: Some(path),
        search_query: None,
    };
    Html(html_render::library(list_to_display))
}

async fn admin_handler(Extension(user): Extension<User>) -> impl IntoResponse {
    info!("get /admin : {user:?}");
    Html(format!(
        "Logged in as admin: {}, role {:?}",
        user.name, user.role
    ))
}

async fn get_root(Extension(user): Extension<Option<User>>) -> impl IntoResponse {
    info!("get / : as {user:?}");
    match user {
        Some(user) => {
            debug!("user found");
            Html(html_render::homepage(&user))
        }
        None => {
            debug!("no user found, login form");
            Html(html_render::login_form())
        }
    }
}

async fn get_css(Path(path): Path<String>) -> impl IntoResponse {
    info!("get /css/{}", path);
    // TODO include_bytes pour la base ? (cf monit-agregator)
    let css_file_content = fs::read_to_string(format!("src/css/{path}"));
    // TODO tests content pour 200 ?
    match css_file_content {
        Ok(css) => (StatusCode::OK, [(header::CONTENT_TYPE, "text/css")], css).into_response(),
        Err(_) => {
            error!("images {path} not found");
            // TODO true 404
            (StatusCode::NOT_FOUND, "css not found").into_response()
        }
    }
}

async fn get_images(Path(path): Path<String>) -> impl IntoResponse {
    info!("get /images/{}", path);
    // TODO include_bytes pour la base ? (cf monit-agregator)
    // read_to_string if svg instead of svgz
    // https://developer.mozilla.org/en-US/docs/Web/SVG/Tutorial/Getting_Started#a_word_on_web_servers_for_.svgz_files
    let image_file_content = fs::read(format!("src/images/{path}"));
    // TODO tests content pour 200 ?
    match image_file_content {
        Ok(image) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "image/svg+xml")],
            [(header::CONTENT_ENCODING, "gzip")],
            [(header::VARY, "Accept-Encoding")],
            image,
        )
            .into_response(),
        Err(_) => {
            error!("image {path} not found");
            // TODO true 404
            (StatusCode::NOT_FOUND, "image not found").into_response()
        }
    }
}

async fn create_router() -> Router {
    let secret = rand::thread_rng().gen::<[u8; 64]>();
    // TODO MemoryStore KO en prod
    let session_store = MemoryStore::new();
    // TODO cookies options (secure, ttl, ...) :
    // https://docs.rs/axum-sessions/0.4.1/axum_sessions/struct.SessionLayer.html#implementations
    let session_layer = SessionLayer::new(session_store, &secret).with_secure(false);
    let pool = sqlite::create_sqlite_pool().await;
    let user_store = SqliteStore::<User, Role>::new(pool);
    let auth_layer = AuthLayer::new(user_store, &secret);

    Router::new()
        // 🔒 protected 🔒
        .route("/admin", get(admin_handler))
        .route_layer(RequireAuthorizationLayer::<User, Role>::login_with_role(
            Role::Admin..,
        ))
        // 🤔 I don't know why but /library and /library/ are also protected...
        .route("/library", get(library_handler))
        .route("/library/", get(library_handler))
        .route("/library/*path", get(library_handler))
        .route_layer(RequireAuthorizationLayer::<User, Role>::login_with_role(
            Role::User..,
        ))
        .route("/toggle/:flag/:id", get(flag_handler))
        .route_layer(RequireAuthorizationLayer::<User, Role>::login_with_role(
            Role::User..,
        ))
        .route("/bookmarks", get(bookmarks_handler))
        .route_layer(RequireAuthorizationLayer::<User, Role>::login_with_role(
            Role::User..,
        ))
        .route("/search", post(search_handler))
        .route_layer(RequireAuthorizationLayer::<User, Role>::login_with_role(
            Role::User..,
        ))
        .route("/download/:id", get(download_handler))
        .route_layer(RequireAuthorizationLayer::<User, Role>::login_with_role(
            Role::User..,
        ))
        .route("/read/:id/:page", get(reader_handler))
        .route_layer(RequireAuthorizationLayer::<User, Role>::login_with_role(
            Role::User..,
        ))
        .route("/infos/:id", get(infos_handler))
        .route_layer(RequireAuthorizationLayer::<User, Role>::login_with_role(
            Role::User..,
        ))
        .route("/cover/:id", get(cover_handler))
        .route_layer(RequireAuthorizationLayer::<User, Role>::login_with_role(
            Role::User..,
        ))
        // 🔥 UNPROTECTED 🔥
        .route("/", get(get_root))
        .route("/css/*path", get(get_css))
        // UI images, not covers
        .route("/images/*path", get(get_images))
        .route("/login", post(login_handler))
        .route("/logout", get(logout_handler))
        // layers for redirect when not logged
        // see https://github.com/maxcountryman/axum-login/issues/22#issuecomment-1345403733
        .layer(
            ServiceBuilder::new()
                .layer(session_layer)
                .layer(auth_layer)
                .map_response(|response: Response| {
                    if response.status() == StatusCode::UNAUTHORIZED {
                        Redirect::to("/").into_response()
                    } else {
                        response
                    }
                }),
        )
}

use std::net::SocketAddr;
use std::net::SocketAddrV4;
use std::str::FromStr;
pub async fn start_http_server(bind: &str) -> Result<(), Error> {
    info!("start http server on {}", bind);
    // TODO handle error, and default value
    let bind = SocketAddrV4::from_str(bind).unwrap();
    let bind = SocketAddr::from(bind);
    let router = create_router();

    // TODO check si server bien started
    axum::Server::bind(&bind)
        .serve(router.await.into_make_service())
        .await
        .unwrap();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum_test_helper::TestClient;
    use sqlx::{migrate::MigrateDatabase, Sqlite};

    const DB_URL: &str = "sqlite://sqlite/eloran.db";

    #[tokio::test]
    async fn test_login_logout() {
        // init db
        sqlite::init_database().await;
        sqlite::init_users(DB_URL).await;
        // headers
        let headers = "<!DOCTYPE html><html><head><title>Eloran</title><meta charset=\"UTF-8\"><meta name=\"viewport\" content=\"width=device-width\">";
        let css = "<link rel=\"stylesheet\" href=\"/css/eloran.css\"><link rel=\"stylesheet\" href=\"/css/w3.css\"><link rel=\"stylesheet\" href=\"/css/w3-theme-dark-grey.css\">";
        let metas = "<meta http-equiv=\"Cache-Control\" content=\"no-cache, no-store, must-revalidate\"><meta http-equiv=\"Pragma\" content=\"no-cache\"><meta http-equiv=\"Expires\" content=\"0\">";
        let meta_redir_library = "<meta http-equiv=\"refresh\" content=\"0; url='/library'\">";
        let meta_redir_home = "<meta http-equiv=\"refresh\" content=\"0; url='/'\">";
        let body = "</head><body class=\"w3-theme-dark\">";
        // create router
        let router = create_router();
        // root without auth
        let client = TestClient::new(router.await);
        let res = client.get("/").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, format!("{headers}{css}{metas}{body}<h2 id=\"heading\">Eloran</h2><p>Please login :</p><p><form action=\"/login\" method=\"post\"><input type=\"text\" name=\"user\" placeholder=\"username\" required><br><input type=\"password\" name=\"password\" placeholder=\"password\" required><br><input type=\"submit\" value=\"Login\"></form></p></body></html>"));
        // login
        let res = client
            .post("/login")
            .body("user=admin&password=pass123")
            .send()
            .await;
        assert_eq!(res.status(), StatusCode::OK);
        // get cookie
        let res_headers = res.headers();
        assert!(res_headers.contains_key("set-cookie"));
        let cookie = match res_headers.get("set-cookie") {
            Some(cookie) => cookie.clone(),
            None => panic!(),
        };
        assert_eq!(
            res.text().await,
            format!("{headers}{css}{metas}{meta_redir_library}{body}<h2 id=\"heading\">Eloran</h2><p>Successfully logged in as: admin, role Admin</p><p><a href=\"/\">return home</a></p></body></html>"));
        // root with auth
        let res = client.get("/").header("Cookie", &cookie).send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, format!("{headers}{css}{metas}{body}<h2 id=\"heading\">Eloran</h2><div id=\"menu\"><p><a href=\"/library\">library</a> | <a href=\"/bookmarks\">bookmarks</a> | <a href=\"/prefs\">preferences</a> | <a href=\"/admin\">administration</a> | admin (<a href=\"/logout\">logout</a>)</p><form action=\"/search\" method=\"post\"><input type=\"text\" placeholder=\"Search..\" name=\"query\"></form></div><div id=\"home-content\">content</div></body></html>"));
        // logout
        let res = client.get("/logout").header("Cookie", &cookie).send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, format!("{headers}{css}{metas}{meta_redir_home}{body}<h2 id=\"heading\">Eloran</h2><p>Bye admin</p><p><a href=\"/\">return home</a></p></body></html>"));
        // root without auth
        let res = client.get("/").header("Cookie", &cookie).send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, format!("{headers}{css}{metas}{body}<h2 id=\"heading\">Eloran</h2><p>Please login :</p><p><form action=\"/login\" method=\"post\"><input type=\"text\" name=\"user\" placeholder=\"username\" required><br><input type=\"password\" name=\"password\" placeholder=\"password\" required><br><input type=\"submit\" value=\"Login\"></form></p></body></html>"));
        // css error
        let res = client.get("/css/toto").send().await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        let res = client.get("/css/w3.css").send().await;
        let res_headers = match res.headers().get("content-type") {
            Some(header) => header,
            None => panic!(),
        };
        assert_eq!(res_headers, "text/css");
        // delete database
        Sqlite::drop_database(crate::DB_URL);
    }

    #[test]
    fn parse_user_password_test() {
        let body = String::from("user=myuser&password=mypass");
        let (user, password) = parse_credentials(&body);
        assert_eq!(user, String::from("myuser"));
        assert_eq!(password, String::from("mypass"));
    }
}
