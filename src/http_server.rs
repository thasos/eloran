use crate::html_render::{self, login_ok};
use crate::reader;
use crate::scanner::{self, DirectoryInfo, FileInfo, Library};
use crate::sqlite;

use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
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
use serde::Deserialize;
use std::process;
use std::{
    collections::VecDeque,
    fs,
    net::{SocketAddr, SocketAddrV4},
    str::FromStr,
};
use tower::ServiceBuilder;
use urlencoding::decode;

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
impl AuthUser<i64, Role> for User {
    fn get_id(&self) -> i64 {
        self.id
    }
    fn get_password_hash(&self) -> SecretVec<u8> {
        SecretVec::new(self.password_hash.clone().into())
    }
    fn get_role(&self) -> Option<Role> {
        Some(Role::User)
    }
}

type AuthContext = axum_login::extractors::AuthContext<i64, User, SqliteStore<User, Role>, Role>;
type RequireAuth = RequireAuthorizationLayer<i64, User, Role>;

/// Roles
#[derive(Debug, Clone, PartialEq, PartialOrd, sqlx::Type, Default)]
pub enum Role {
    #[default]
    User,
    Admin,
}

// TODO use struct, like new_user_handler()
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

fn error_handler() -> Html<String> {
    Html(html_render::simple_message(
        String::from("server error, please see logs"),
        String::from("/"),
    ))
}
async fn reading_handler(Extension(user): Extension<User>) -> impl IntoResponse {
    info!("get /reading : {}", &user.name);
    match sqlite::create_sqlite_pool().await {
        Ok(conn) => {
            // search files
            let mut files_results = sqlite::get_reading_files_from_user_id(&user.id, &conn).await;
            files_results.sort();
            // add status (read, bookmark)
            let user = sqlite::get_user(Some(&user.name), None, &conn).await;
            let user = user.first().unwrap();
            let mut files_results_with_status: Vec<(FileInfo, bool, bool)> =
                Vec::with_capacity(files_results.capacity());
            for file in files_results {
                let bookmark_status =
                    sqlite::get_flag_status("bookmark", user.id, &file.id, &conn).await;
                let read_status =
                    sqlite::get_flag_status("read_status", user.id, &file.id, &conn).await;
                files_results_with_status.push((file, bookmark_status, read_status));
            }
            // lib path
            let library_path = sqlite::get_library(None, None, &conn).await;
            let empty_library = Library::default();
            let library_path = match library_path.first() {
                Some(library_path) => library_path,
                None => &empty_library,
            }
            .to_owned();
            conn.close().await;
            // response
            let list_to_display = html_render::LibraryDisplay {
                user: user.clone(),
                directories_list: Vec::with_capacity(0),
                files_list: files_results_with_status,
                library_id: None,
                library_path: library_path.path,
                current_path: None,
                search_query: None,
            };
            Html(html_render::library_display(list_to_display))
        }
        Err(_) => error_handler(),
    }
}

async fn bookmarks_handler(Extension(user): Extension<User>) -> impl IntoResponse {
    info!("get /bookmarks : {}", &user.name);
    match sqlite::create_sqlite_pool().await {
        Ok(conn) => {
            // search files
            let mut files_results = sqlite::bookmarks_for_user_id(user.id, &conn).await;
            files_results.sort();
            // add status (read, bookmark)
            let user = sqlite::get_user(Some(&user.name), None, &conn).await;
            let user = user.first().unwrap();
            let mut files_results_with_status: Vec<(FileInfo, bool, bool)> =
                Vec::with_capacity(files_results.capacity());
            for file in files_results {
                let bookmark_status =
                    sqlite::get_flag_status("bookmark", user.id, &file.id, &conn).await;
                let read_status =
                    sqlite::get_flag_status("read_status", user.id, &file.id, &conn).await;
                files_results_with_status.push((file, bookmark_status, read_status));
            }
            // lib path
            let library_path = sqlite::get_library(None, None, &conn).await;
            // let library_path = library_path.first().unwrap().to_owned();
            let library_path = match library_path.first() {
                Some(library_path) => library_path.to_owned(),
                None => Library::new(),
            };
            conn.close().await;
            // response
            let list_to_display = html_render::LibraryDisplay {
                user: user.clone(),
                directories_list: Vec::with_capacity(0),
                files_list: files_results_with_status,
                library_id: None,
                library_path: library_path.path,
                current_path: None,
                search_query: None,
            };
            Html(html_render::library_display(list_to_display))
        }
        Err(_) => error_handler(),
    }
}

// TODO use struct, like new_user_handler()
async fn search_handler(Extension(user): Extension<User>, query: String) -> impl IntoResponse {
    info!("get /search : {}", &query);
    // body string is `query=search_string`, we need only the `search_string`
    let query = query.strip_prefix("query=").unwrap();
    let query = &query.replace('+', " ");
    match sqlite::create_sqlite_pool().await {
        Ok(conn) => {
            // search files
            let mut files_results = sqlite::search_file_from_string(query, &conn).await;
            files_results.sort();
            // add status (read, bookmark)
            let user = sqlite::get_user(Some(&user.name), None, &conn).await;
            let user = user.first().unwrap();
            let mut files_results_with_status: Vec<(FileInfo, bool, bool)> =
                Vec::with_capacity(files_results.capacity());
            for file in files_results {
                let bookmark_status =
                    sqlite::get_flag_status("bookmark", user.id, &file.id, &conn).await;
                let read_status =
                    sqlite::get_flag_status("read_status", user.id, &file.id, &conn).await;
                files_results_with_status.push((file, bookmark_status, read_status));
            }
            // search dirs
            let mut directories_results = sqlite::search_directory_from_string(query, &conn).await;
            directories_results.sort();
            // lib path
            let library_path = sqlite::get_library(None, None, &conn).await;
            let library_path = library_path.first().unwrap().to_owned();
            conn.close().await;
            // response
            let list_to_display = html_render::LibraryDisplay {
                user: user.clone(),
                directories_list: directories_results,
                files_list: files_results_with_status,
                library_id: None,
                library_path: library_path.path,
                current_path: None,
                search_query: Some(query.to_string()),
            };
            Html(html_render::library_display(list_to_display))
        }
        Err(_) => error_handler(),
    }
}

// TODO use struct, like new_user_handler()
async fn login_handler(mut auth: AuthContext, body: String) -> impl IntoResponse {
    info!("get /login");
    let (username, password) = parse_credentials(&body);
    match sqlite::create_sqlite_pool().await {
        Ok(conn) => {
            // get user from db
            // TODO hash password
            Html({
                let login_response = match sqlx::query_as(
                    "SELECT * FROM users WHERE name = ? AND password_hash = ?;",
                )
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
        Err(_) => error_handler(),
    }
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
    Path(file_id): Path<String>,
) -> impl IntoResponse {
    match sqlite::create_sqlite_pool().await {
        Ok(conn) => {
            let file = match sqlite::get_files_from_file_id(&file_id, &conn).await {
                Some(file) => file,
                None => FileInfo::new(),
            };
            // path for up link
            let library_name = &file.library_name;
            let library_vec = sqlite::get_library(Some(library_name), None, &conn).await;
            let library = if let Some(first_library) = library_vec.first() {
                first_library.to_owned()
            } else {
                Library::new()
            };
            let library_path = &library.path;
            let up_link = file
                .parent_path
                .replace(library_path, &format!("/library/{library_name}"));
            if file.scan_me == 1 {
                scanner::extract_all(&file, &conn).await;
            }
            // we need user_id for bookmark and read status
            let user = sqlite::get_user(Some(&user.name), None, &conn).await;
            let user = user.first().unwrap();
            let bookmark_status =
                sqlite::get_flag_status("bookmark", user.id, &file.id, &conn).await;
            let read_status =
                sqlite::get_flag_status("read_status", user.id, &file.id, &conn).await;

            let current_page =
                sqlite::get_current_page_from_file_id(user.id, &file.id, &conn).await;

            conn.close().await;
            Html(html_render::file_info(
                user,
                &file,
                current_page,
                bookmark_status,
                read_status,
                up_link,
            ))
        }
        Err(_) => error_handler(),
    }
}

/// add/remove flag (bookmark or read status) of a file for a user
async fn flag_handler(
    Extension(user): Extension<User>,
    Path((flag, file_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match sqlite::create_sqlite_pool().await {
        Ok(conn) => {
            let user = sqlite::get_user(Some(&user.name), None, &conn).await;
            let user = user.first().unwrap();
            let flag_status = sqlite::set_flag_status(&flag, user.id, &file_id, &conn).await;
            conn.close().await;
            Html(html_render::flag_toggle(user, flag_status, &file_id, &flag))
        }
        Err(_) => error_handler(),
    }
}

async fn cover_handler(
    Extension(_user): Extension<User>,
    Path(file_id): Path<String>,
) -> impl IntoResponse {
    match sqlite::create_sqlite_pool().await {
        Ok(conn) => {
            let file = match sqlite::get_files_from_file_id(&file_id, &conn).await {
                Some(file) => file,
                None => FileInfo::new(),
            };
            debug!("get /cover/{}", file_id);
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
        Err(_) => error_handler().into_response(),
    }
}

// TODO filename...
async fn download_handler(
    Extension(user): Extension<User>,
    Path(file_id): Path<String>,
) -> impl IntoResponse {
    match sqlite::create_sqlite_pool().await {
        Ok(conn) => {
            info!("get /download/{} : {}", &file_id, &user.name);
            let file = match sqlite::get_files_from_file_id(&file_id, &conn).await {
                Some(file) => file,
                None => FileInfo::new(),
            };
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
        Err(_) => error_handler().into_response(),
    }
}

// TODO return image, origin or small
async fn comic_page_handler(
    Extension(user): Extension<User>,
    Path((file_id, page, size)): Path<(String, i32, String)>,
) -> impl IntoResponse {
    info!("get /reader/{} (page {}) : {}", &file_id, &page, &user.name);
    match sqlite::create_sqlite_pool().await {
        Ok(conn) => {
            let file = match sqlite::get_files_from_file_id(&file_id, &conn).await {
                Some(file) => file,
                None => FileInfo::new(),
            };
            match reader::get_comic_page(&file, page, &size).await {
                Some(comic_board) => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "image/jpeg")],
                    comic_board,
                )
                    .into_response(),
                None => "unable to get image".into_response(),
            }
        }
        Err(_) => error_handler().into_response(),
    }
}

async fn reader_handler(
    Extension(user): Extension<User>,
    Path((file_id, page)): Path<(String, i32)>,
) -> impl IntoResponse {
    // TODO set current page to 0 if not provided ?
    // let page: i32 = page.unwrap_or(0);
    match sqlite::create_sqlite_pool().await {
        Ok(conn) => {
            info!("get /reader/{} (page {}) : {}", &file_id, &page, &user.name);
            let file = match sqlite::get_files_from_file_id(&file_id, &conn).await {
                Some(file) => file,
                None => FileInfo::new(),
            };
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
            // set page at current_page
            sqlite::set_current_page_for_file_id(&file.id, &user.id, &page, &conn).await;
            // remove from reading table if last page
            if page == file.total_pages - 1 {
                sqlite::remove_file_id_from_reading(&file.id, &user.id, &conn).await;
                // and mark as read if needed
                if !sqlite::get_flag_status("read_status", user.id, &file.id, &conn).await {
                    let _ = sqlite::set_flag_status("read_status", user.id, &file.id, &conn).await;
                }
            }

            let response = match file.format.as_str() {
                "epub" => {
                    let epub_reader = reader::epub(&file, page).await;
                    Html(html_render::ebook_reader(&user, &file, &epub_reader, page))
                        .into_response()
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
                            warn!(
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
                    // let comic_reader = reader::comics(&file, page).await;
                    // Html(html_render::ebook_reader(&user, &file, &comic_reader, page)).into_response()
                    Html(html_render::comic_reader(&user, &file, page)).into_response()
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
        Err(_) => error_handler().into_response(),
    }
}

async fn admin_handler(Extension(user): Extension<User>) -> impl IntoResponse {
    info!("get /admin : {}", &user.name);
    if user.role == Role::Admin {
        match sqlite::create_sqlite_pool().await {
            Ok(conn) => {
                // libraries
                let library_list = sqlite::get_library(None, None, &conn).await;
                // users
                let user_list = sqlite::get_user(None, None, &conn).await;
                // render
                Html(html_render::admin(&user, library_list, user_list)).into_response()
            }
            Err(_) => error_handler().into_response(),
        }
    } else {
        // TODO better display, and redirect to `/` after 3s
        Html("You are not allowed to see this page").into_response()
    }
}

// TODO
async fn prefs_handler(Extension(user): Extension<User>) -> impl IntoResponse {
    info!("get /prefs : {}", &user.name);
    Html(html_render::prefs(&user)).into_response()
}

// TODO call add_library fn...
// TODO use struct, like new_user_handler()
async fn new_library_handler(Extension(user): Extension<User>, path: String) -> impl IntoResponse {
    // only admin
    if user.role == Role::Admin {
        // retrieve path from body
        let path = path.split('=').last().unwrap_or("");
        use std::borrow::Cow;
        let decoded_path = match decode(path) {
            Ok(path) => path,
            Err(_) => Cow::from(""),
        }
        .replace('+', " ")
        .trim_end_matches('/')
        .to_string();
        // following fn wants a vec
        let vec_decoded_path = vec![decoded_path.to_owned()];
        // add the new path in db
        sqlite::create_library_path(vec_decoded_path).await;
        // return confirmation message
        // TODO render
        Html(format!(
            "new library added, path :  {}<br /><a href=\"/\">return</a>",
            decoded_path
        ))
        .into_response()
    } else {
        unauthorized_admin_response().into_response()
    }
}

#[derive(Deserialize)]
struct FormUser {
    name: String,
    password: String,
    is_admin: Option<String>,
}
async fn new_user_handler(
    Extension(user): Extension<User>,
    Form(body): Form<FormUser>,
) -> impl IntoResponse {
    if user.role == Role::Admin {
        // TODO check if name already exists
        let new_user = User {
            name: body.name,
            password_hash: body.password,
            role: {
                if let Some(box_content) = body.is_admin {
                    if box_content == "on" {
                        Role::Admin
                    } else {
                        Role::User
                    }
                } else {
                    Role::User
                }
            },
            ..User::default()
        };
        match sqlite::create_sqlite_pool().await {
            Ok(conn) => {
                sqlite::create_user(&new_user, &conn).await;
                Html(html_render::simple_message(
                    String::from("user created"),
                    String::from("/admin"),
                ))
                .into_response()
            }
            Err(_) => error_handler().into_response(),
        }
    } else {
        Html(html_render::simple_message(
            String::from("your are not allowed to create users"),
            String::from("/"),
        ))
        .into_response()
    }
}

#[derive(Deserialize)]
struct FormUpdateUser {
    password: String,
    is_admin: Option<String>,
    update: Option<String>,
    delete: Option<String>,
}
async fn change_user_handler(
    Extension(user): Extension<User>,
    Path(user_id): Path<String>,
    Form(body): Form<FormUpdateUser>,
) -> impl IntoResponse {
    if user.role == Role::Admin {
        match sqlite::create_sqlite_pool().await {
            Ok(conn) => {
                let check_user = sqlite::get_user(None, Some(&user_id), &conn).await;
                if check_user.is_empty() {
                    Html(html_render::simple_message(
                        format!("user id {} does not exists", &user_id),
                        String::from("/admin"),
                    ))
                    .into_response()
                } else {
                    let mut user_to_update = check_user.first().unwrap().to_owned();
                    if body.delete.is_some() && user_to_update.id != 1 {
                        sqlite::delete_user(&user_to_update, &conn).await;
                        Html(html_render::simple_message(
                            format!("user {} deleted", &user_to_update.name),
                            String::from("/admin"),
                        ))
                        .into_response()
                    } else if body.update.is_some() {
                        if !body.password.is_empty() {
                            user_to_update.password_hash = body.password;
                        }
                        if let Some(is_admin) = body.is_admin {
                            if is_admin.as_str() == "on" {
                                user_to_update.role = Role::Admin;
                            }
                        } else if user_to_update.id != 1 {
                            user_to_update.role = Role::User;
                        }
                        sqlite::update_user(&user_to_update, &conn).await;
                        Html(html_render::simple_message(
                            format!("user {} updated", &user_to_update.name),
                            String::from("/admin"),
                        ))
                        .into_response()
                    } else {
                        Html(html_render::simple_message(
                            String::from("you can't delete admin account"),
                            String::from("/admin"),
                        ))
                        .into_response()
                    }
                }
            }
            Err(_) => error_handler().into_response(),
        }
    } else {
        Html(html_render::simple_message(
            String::from("your are not allowed to modify users"),
            String::from("/"),
        ))
        .into_response()
    }
}

// TODO admin only and call delete_library fn...
async fn admin_library_handler(
    Extension(user): Extension<User>,
    Path(library_id): Path<String>,
    // TODO use struct, like new_user_handler()
    body: String,
) -> impl IntoResponse {
    // only admin
    if user.role == Role::Admin {
        let vec_body: Vec<&str> = body.split('=').collect();
        let option = vec_body.first().unwrap_or(&"").to_string();
        let _value = vec_body.last().unwrap_or(&"").to_string();
        match sqlite::create_sqlite_pool().await {
            Ok(conn) => {
                match option.as_str() {
                    "delete" => {
                        let library = sqlite::get_library(None, Some(&library_id), &conn).await;
                        sqlite::delete_library_from_id(&library, &conn).await;
                        // TODO delete in tables `covers`, `directories` and `reading`
                        sqlite::delete_files_from_library(&library, &conn).await;
                        Html(format!("TODO : delete lib id = {library_id} (<a href=\"/admin\">return to admin panel</a>)")).into_response()
                    }
                    "full_rescan" => {
                        match sqlite::get_library(None, Some(&library_id), &conn)
                            .await
                            .first()
                        {
                            Some(library) => {
                                scanner::launch_scan(library, &conn).await.ok();
                                Html(format!(
                                    "library {} scanned (<a href=\"/admin\">return to admin panel</a>)",
                                    &library.name
                                ))
                                .into_response()
                            }
                            None => Html("unable to find library in database").into_response(),
                        }
                    }
                    "covers" => Html(format!("TODO : lib id = {library_id}, covers flag toggle (<a href=\"/admin\">return to admin panel</a>)"))
                        .into_response(),
                    _ => Html("TODO : error : unknow option").into_response(),
                }
            }
            Err(_) => error_handler().into_response(),
        }
    } else {
        unauthorized_admin_response().into_response()
    }
}

// TODO better display, and redirect to `/` after 3s
fn unauthorized_admin_response() -> Html<String> {
    Html(String::from("You are not allowed to see this page"))
}

async fn library_handler(
    Extension(user): Extension<User>,
    path: Option<Path<String>>,
) -> impl IntoResponse {
    match sqlite::create_sqlite_pool().await {
        Ok(conn) => {
            // sub_path is the string after the first `/`
            let sub_path = match &path {
                Some(path) => format!("/{}", path.as_str()),
                None => String::new(),
            };
            info!("get /library{} : {}", &sub_path, &user.name);

            // if sub_path is empty : `/library` is called
            // we must print all libraries
            let list_to_display = if sub_path.is_empty() {
                // construct library list
                let library_list: Vec<Library> = {
                    match sqlx::query_as("SELECT * FROM core;").fetch_all(&conn).await {
                        Ok(library_list_rows) => library_list_rows,
                        Err(e) => {
                            warn!("empty library : {}", e);
                            Vec::new()
                        }
                    }
                };

                let mut library_as_directories_list: Vec<DirectoryInfo> = Vec::new();
                for library in library_list {
                    let library_as_dir = DirectoryInfo {
                        id: library.id.to_string(),
                        name: library.name.trim_start_matches('/').to_string(),
                        parent_path: "".to_string(),
                        file_number: None,
                    };
                    library_as_directories_list.push(library_as_dir);
                }

                library_as_directories_list.sort();
                html_render::LibraryDisplay {
                    user: user.clone(),
                    directories_list: library_as_directories_list,
                    files_list: Vec::new(),
                    library_id: None,
                    library_path: "/".to_string(),
                    current_path: Some(sub_path.clone()),
                    search_query: None,
                }
            // if sub_path is not empty, we are in a specific library (`/library/foo`)
            } else {
                // retrieve library name from path begining
                let (library_name, path_end) = match &path {
                    // `/library/foo/bar/baz` become :
                    // - library_name : `foo`
                    // - path_rest : `bar/baz`
                    Some(path) => {
                        let path = path.to_string();
                        let mut vec_splitted_path: VecDeque<&str> = path.split('/').collect();
                        let library_name = vec_splitted_path[0].to_string();
                        vec_splitted_path.pop_front();
                        let end: String = vec_splitted_path
                            .iter()
                            .map(|s| "/".to_string() + s)
                            .collect();
                        (library_name, end)
                    }
                    None => ("".to_string(), "".to_string()),
                };

                // TODO fix this ugly block... it's a simple `let library = match....`
                // retrieve true parent_path on disk from library name
                let libraries_vec = sqlite::get_library(Some(&library_name), None, &conn).await;
                let query_parent_path = match libraries_vec.first() {
                    Some(path) => format!("{}{}", path.path.to_owned(), path_end),
                    None => {
                        let msg = "an empty library path should not happen, you should force a full rescan";
                        warn!("{msg}");
                        msg.to_string()
                    }
                };
                // 🤮 remove this block, see above TODO
                let library = match libraries_vec.first() {
                    Some(library) => library.clone(),
                    None => Library::default(),
                };

                // we need user_id for bookmark and read status
                let user = sqlite::get_user(Some(&user.name), None, &conn).await;
                let user = user.first().unwrap();

                // construct lists
                let mut files_list_with_status: Vec<(FileInfo, bool, bool)> = {
                    // TODO pagination ? set limit in conf
                    let files_list: Vec<FileInfo> =
                        match sqlx::query_as("SELECT * FROM files WHERE parent_path = ?;")
                            .bind(&query_parent_path)
                            .fetch_all(&conn)
                            .await
                        {
                            Ok(files_list) => files_list,
                            Err(e) => {
                                warn!("empty library : {}", e);
                                let empty_list: Vec<FileInfo> = Vec::with_capacity(0);
                                empty_list
                            }
                        };
                    // add bookmark and read status to the list
                    let mut files_list_with_status: Vec<(FileInfo, bool, bool)> =
                        Vec::with_capacity(files_list.capacity());
                    for file in files_list {
                        let bookmark_status =
                            sqlite::get_flag_status("bookmark", user.id, &file.id, &conn).await;
                        let read_status =
                            sqlite::get_flag_status("read_status", user.id, &file.id, &conn).await;
                        files_list_with_status.push((file, bookmark_status, read_status));
                    }
                    files_list_with_status
                };
                files_list_with_status.sort();

                let mut directories_list: Vec<DirectoryInfo> = {
                    info!("get /library{} : {}", &sub_path, &user.name);
                    // TODO set limit in conf
                    let directories_list: Vec<DirectoryInfo> =
                        match sqlx::query_as("SELECT * FROM directories WHERE parent_path = ?;")
                            .bind(&query_parent_path)
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
                html_render::LibraryDisplay {
                    user: user.clone(),
                    directories_list,
                    files_list: files_list_with_status,
                    library_id: Some(library.id),
                    library_path: query_parent_path.to_string(),
                    current_path: Some(sub_path),
                    search_query: None,
                }
            };
            Html(html_render::library_display(list_to_display))
        }
        Err(_) => error_handler(),
    }
}

// async fn get_root(Extension(user): Extension<User>) -> impl IntoResponse {
async fn get_root(auth: AuthContext) -> impl IntoResponse {
    match auth.current_user {
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
    info!("get /css/{}", &path);
    // TODO include_bytes pour la base ? (cf monit-agregator)
    let css_file_content = fs::read_to_string(format!("src/css/{path}"));
    // TODO tests content pour 200 ?
    match css_file_content {
        Ok(css) => (StatusCode::OK, [(header::CONTENT_TYPE, "text/css")], css).into_response(),
        Err(_) => {
            error!("css {path} not found");
            // TODO true 404
            (StatusCode::NOT_FOUND, "css not found").into_response()
        }
    }
}

async fn get_images(Path(path): Path<String>) -> impl IntoResponse {
    info!("get /images/{}", &path);
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

// TODO useless ?
// async fn fallback() -> impl IntoResponse {
//     Redirect::to("/").into_response()
// }

async fn create_router() -> Router {
    let secret = rand::thread_rng().gen::<[u8; 64]>();
    // TODO MemoryStore KO in prod
    let session_store = MemoryStore::new();
    // --
    // test with https://docs.rs/async-sqlx-session/
    // a restart still destroy the session... why ?
    // I see sessions in sqlite : `SELECT * FROM async_sessions ;`
    // --
    // let session_store = SqliteSessionStore::new(crate::DB_URL)
    //     .await
    //     .expect("unable to connect to database to create auth session");
    // session_store
    //     .migrate()
    //     .await
    //     .expect("unable to create auth session in database");

    // TODO cookies options (secure, ttl, ...) :
    // https://docs.rs/axum-sessions/0.4.1/axum_sessions/struct.SessionLayer.html#implementations
    let session_layer = SessionLayer::new(session_store, &secret).with_secure(false);
    // TODO true sqlite store
    // see https://github.com/maxcountryman/axum-login/blob/main/examples/oauth/src/main.rs
    match sqlite::create_sqlite_pool().await {
        Ok(pool) => {
            let user_store = SqliteStore::<User, Role>::new(pool);
            let auth_layer = AuthLayer::new(user_store, &secret);

            Router::new()
                // 🔒🔒🔒 ADMIN PROTECTED 🔒🔒🔒
                .route("/admin", get(admin_handler))
                .route("/admin/library/:library_id", post(admin_library_handler))
                .route("/admin/library/new", post(new_library_handler))
                .route("/admin/user/:user_id", post(change_user_handler))
                .route("/admin/user/new", post(new_user_handler))
                // TODO does not work here, 401 despite logged as Admin...
                // without this protection, a Role::User can go to the route /admin, but a check is done in the handler
                // so it is a minor risk
                // .route_layer(RequireAuth::login_with_role(Role::Admin..))
                // 🔒🔒🔒 PROTECTED 🔒🔒🔒
                .route("/prefs", get(prefs_handler))
                .route("/library", get(library_handler))
                .route("/library/*path", get(library_handler))
                .route("/toggle/:flag/:id", get(flag_handler))
                .route("/bookmarks", get(bookmarks_handler))
                .route("/reading", get(reading_handler))
                .route("/search", post(search_handler))
                .route("/download/:file_id", get(download_handler))
                .route("/read/:file_id/:page", get(reader_handler))
                .route("/comic_page/:file_id/:page/:size", get(comic_page_handler))
                .route("/infos/:file_id", get(infos_handler))
                .route("/cover/:file_id", get(cover_handler))
                .route_layer(RequireAuth::login_with_role(Role::User..))
                // 🔥🔥🔥 UNPROTECTED 🔥🔥🔥
                .route("/", get(get_root))
                .route("/css/*path", get(get_css))
                .route("/images/*path", get(get_images)) // ⚠️  UI images, not covers
                .route("/login", post(login_handler))
                .route("/logout", get(logout_handler))
                // TODO useless ?
                // .fallback(fallback)
                // ---
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
        // TODO ça c'est moisi...
        Err(_) => {
            error!("unable to connect to database, exiting");
            process::exit(1);
        }
    }
}

pub async fn start_http_server(bind: &str) -> Result<(), String> {
    info!("start http server on {}", bind);
    // TODO handle error, and default value
    let bind = SocketAddrV4::from_str(bind).unwrap();
    let bind = SocketAddr::from(bind);
    let router = create_router();

    // TODO trim trailing slash
    // see https://docs.rs/tower-http/latest/tower_http/normalize_path/struct.NormalizePathLayer.html?search=trim_trailing_slash#method.trim_trailing_slash
    // and
    // https://stackoverflow.com/questions/75355826/route-paths-with-or-without-of-trailing-slashes-in-rust-axum

    // TODO check si server bien started
    axum::Server::bind(&bind)
        .serve(router.await.into_make_service())
        .await
        .expect("unable to bind http server");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum_test_helper::TestClient;
    use sqlx::{migrate::MigrateDatabase, Sqlite};

    #[tokio::test]
    async fn test_login_logout() {
        // init db
        let _ = sqlite::init_database().await;
        sqlite::init_default_users().await;
        // create router
        let router = create_router();
        // root without auth
        let client = TestClient::new(router.await);
        let res = client.get("/").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        insta::assert_yaml_snapshot!(res.text().await);
        // login
        let res = client
            .post("/login")
            .body("user=admin&password=admin")
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
        insta::assert_yaml_snapshot!(res.text().await);
        // root with auth
        let res = client.get("/").header("Cookie", &cookie).send().await;
        assert_eq!(res.status(), StatusCode::OK);
        insta::assert_yaml_snapshot!(res.text().await);
        // logout
        let res = client.get("/logout").header("Cookie", &cookie).send().await;
        assert_eq!(res.status(), StatusCode::OK);
        insta::assert_yaml_snapshot!(res.text().await);
        // root without auth
        let res = client.get("/").header("Cookie", &cookie).send().await;
        assert_eq!(res.status(), StatusCode::OK);
        insta::assert_yaml_snapshot!(res.text().await);
        // css error
        let res = client.get("/css/not_found").send().await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        let res = client.get("/css/w3.css").send().await;
        let res_headers = match res.headers().get("content-type") {
            Some(header) => header,
            None => panic!(),
        };
        assert_eq!(res_headers, "text/css");
        // delete database
        let _ = Sqlite::drop_database(crate::DB_URL).await;
    }

    #[test]
    fn parse_user_password_test() {
        let body = String::from("user=myuser&password=mypass");
        let (user, password) = parse_credentials(&body);
        assert_eq!(user, String::from("myuser"));
        assert_eq!(password, String::from("mypass"));
    }
}
