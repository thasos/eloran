use crate::html_render;
use crate::reader;
use crate::scanner::{self, DirectoryInfo, FileInfo, Library};
use crate::sqlite;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Router,
};
use axum_login::{
    login_required,
    tower_sessions::{Expiry, MemoryStore, SessionManagerLayer},
    AuthManagerLayerBuilder,
};
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, fs, process};
use time::Duration;
use tower::ServiceBuilder;
use urlencoding::decode;

// struct DatabaseConnection(sqlx::pool::Pool<sqlx::Sqlite>);

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

/// Roles
#[derive(Debug, Clone, PartialEq, PartialOrd, Default, sqlx::Type)]
pub enum Role {
    #[default]
    User,
    Admin,
}

fn error_handler() -> Html<String> {
    Html(html_render::simple_message(
        "server error, please see logs",
        None,
    ))
}

async fn reading_handler(auth_session: AuthSession) -> impl IntoResponse {
    match auth_session.user {
        Some(user) => {
            info!("get /reading : {}", &user.name);
            match sqlite::create_sqlite_pool().await {
                Ok(conn) => {
                    // search files
                    let mut files_results =
                        sqlite::get_reading_files_from_user_id(&user.id, &conn).await;
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
        None => unauthorized_response(),
    }
}

async fn bookmarks_handler(auth_session: AuthSession) -> impl IntoResponse {
    match auth_session.user {
        Some(user) => {
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
        None => unauthorized_response(),
    }
}

// TODO use struct, like new_user_handler()
async fn search_handler(auth_session: AuthSession, query: String) -> impl IntoResponse {
    match auth_session.user {
        Some(user) => {
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
                    let mut directories_results =
                        sqlite::search_directory_from_string(query, &conn).await;
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
        None => unauthorized_response(),
    }
}

async fn login_handler(
    mut auth_session: AuthSession,
    Form(creds): Form<Credentials>,
) -> impl IntoResponse {
    info!("get /login");
    let user = match auth_session.authenticate(creds.clone()).await {
        Ok(Some(user)) => user,
        Ok(None) => return authent_error().into_response(),
        Err(_) => return error_handler().into_response(),
    };
    if auth_session.login(&user).await.is_err() {
        return error_handler().into_response();
    }
    if let Some(ref next) = creds.next {
        Redirect::to(next).into_response()
    } else {
        Redirect::to("/library").into_response()
    }
}

async fn logout_handler(mut auth_session: AuthSession) -> impl IntoResponse {
    info!("get /logout");
    match auth_session.logout().await {
        Ok(_) => {
            debug!("user found, logout");
            Html(html_render::logout())
        }
        Err(e) => {
            warn!("can't logout ! : {e}");
            error_handler()
        }
    }
}

// #[axum::debug_handler]
// TODO link "previous page" or folder of publication
async fn infos_handler(
    auth_session: AuthSession,
    Path(file_id): Path<String>,
) -> impl IntoResponse {
    match auth_session.user {
        Some(user) => {
            match sqlite::create_sqlite_pool().await {
                Ok(conn) => {
                    // if the file is not found in database, create new
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
                    // scan file if needed
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
        None => unauthorized_response(),
    }
}

/// add/remove flag (bookmark or read status) of a file for a user
async fn flag_handler(
    auth_session: AuthSession,
    Path((flag, file_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match auth_session.user {
        Some(user) => match sqlite::create_sqlite_pool().await {
            Ok(conn) => {
                let user = sqlite::get_user(Some(&user.name), None, &conn).await;
                let user = user.first().unwrap();
                let flag_status = sqlite::set_flag_status(&flag, user.id, &file_id, &conn).await;
                conn.close().await;
                Html(html_render::flag_toggle(user, flag_status, &file_id, &flag))
            }
            Err(_) => error_handler(),
        },
        None => unauthorized_response(),
    }
}

async fn cover_handler(
    auth_session: AuthSession,
    Path(file_id): Path<String>,
) -> impl IntoResponse {
    match auth_session.user {
        Some(_user) => {
            match sqlite::create_sqlite_pool().await {
                Ok(conn) => {
                    let file = match sqlite::get_files_from_file_id(&file_id, &conn).await {
                        Some(file) => file,
                        None => FileInfo::new(),
                    };
                    debug!("get /cover/{}", file_id);
                    // defaut cover definition
                    let default_cover = {
                        let image_file_content = fs::read("images/green_book.svgz");
                        match image_file_content {
                            Ok(image) => (
                                StatusCode::OK,
                                [
                                    (header::CONTENT_TYPE, "image/svg+xml"),
                                    (header::CONTENT_ENCODING, "gzip"),
                                    (header::VARY, "Accept-Encoding"),
                                    (header::CACHE_CONTROL, "public, max-age=604800"),
                                ],
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
                    // return default cover if problem with database or cover empty or not supported format
                    match file.format.as_str() {
                        "epub" | "pdf" | "cbz" | "cbr" | "cb7" => {
                            // get cover from database
                            let u8_cover = sqlite::get_cover_from_id(&file, &conn).await;
                            conn.close().await;
                            match u8_cover {
                                Some(cover) => {
                                    if !cover.is_empty() {
                                        (
                                            StatusCode::OK,
                                            [
                                                (header::CONTENT_TYPE, "image/jpeg"),
                                                (header::CACHE_CONTROL, "no-cache"),
                                            ],
                                            cover,
                                        )
                                            .into_response()
                                    } else {
                                        // cover empty
                                        default_cover
                                    }
                                }
                                // unable to get cover from database
                                None => default_cover,
                            }
                        }
                        // format not suupported
                        _ => default_cover,
                    }
                }
                Err(_) => error_handler().into_response(),
            }
        }
        None => unauthorized_response().into_response(),
    }
}

async fn download_handler(
    auth_session: AuthSession,
    Path(file_id): Path<String>,
) -> impl IntoResponse {
    match auth_session.user {
        Some(user) => {
            match sqlite::create_sqlite_pool().await {
                Ok(conn) => {
                    info!("get /download/{} : {}", &file_id, &user.name);
                    let file = match sqlite::get_files_from_file_id(&file_id, &conn).await {
                        Some(file) => file,
                        None => FileInfo::new(),
                    };
                    let full_path = format!("{}/{}", file.parent_path, file.name);
                    // possible content-types : https://www.iana.org/assignments/media-types/media-types.xhtml
                    let content_type = match file.format.as_str() {
                        "epub" => "application/epub+zip",
                        "pdf" => "application/pdf",
                        "cbz" => "application/vnd.comicbook+zip",
                        "cbr" => "application/vnd.comicbook-rar",
                        _ => "",
                    };
                    if let Ok(file_content) = fs::read(full_path) {
                        (
                            StatusCode::OK,
                            [
                                (header::CONTENT_TYPE, content_type),
                                (header::CACHE_CONTROL, "no-cache"),
                            ],
                            [(
                                header::CONTENT_DISPOSITION,
                                format!("attachment; filename=\"{}\"", &file.name),
                            )],
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
        None => unauthorized_response().into_response(),
    }
}

// TODO return image, origin or small
async fn comic_page_handler(
    auth_session: AuthSession,
    Path((file_id, page, size)): Path<(String, i32, String)>,
) -> impl IntoResponse {
    match auth_session.user {
        Some(user) => {
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
                            [
                                (header::CONTENT_TYPE, "image/jpeg"),
                                (header::CACHE_CONTROL, "no-cache"),
                            ],
                            comic_board,
                        )
                            .into_response(),
                        None => Html(html_render::simple_message(
                            "unable to get image",
                            Some(&format!("/reader/{}", &file_id)),
                        ))
                        .into_response(),
                    }
                }
                Err(_) => error_handler().into_response(),
            }
        }
        None => unauthorized_response().into_response(),
    }
}

async fn reader_handler(
    auth_session: AuthSession,
    Path((file_id, page)): Path<(String, i32)>,
) -> impl IntoResponse {
    // TODO set current page to 0 if not provided ?
    // let page: i32 = page.unwrap_or(0);
    match auth_session.user {
        Some(user) => {
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
                            let _ =
                                sqlite::set_flag_status("read_status", user.id, &file.id, &conn)
                                    .await;
                        }
                    }

                    let response = match file.format.as_str() {
                        "epub" => {
                            let epub_reader = reader::epub(&file, page).await;
                            Html(html_render::ebook_reader(&user, &file, &epub_reader, page))
                                .into_response()
                        }
                        "pdf" => {
                            let pdf_file =
                                fs::read(format!("{}/{}", &file.parent_path, &file.name));
                            match pdf_file {
                                Ok(pdf_file) => (
                                    StatusCode::OK,
                                    [
                                        (header::CONTENT_TYPE, "application/pdf"),
                                        (header::CACHE_CONTROL, "no-cache"),
                                    ],
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
                        _ => Html(html_render::simple_message("no yet supported", None))
                            .into_response(),
                    };
                    conn.close().await;
                    response
                }
                Err(_) => error_handler().into_response(),
            }
        }
        None => unauthorized_response().into_response(),
    }
}

#[axum::debug_handler]
async fn admin_handler(
    auth_session: AuthSession,
    // DatabaseConnection(conn): DatabaseConnection,
) -> impl IntoResponse {
    match auth_session.user {
        Some(user) => {
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
        None => unauthorized_response().into_response(),
    }
}

// TODO
async fn prefs_handler(auth_session: AuthSession) -> impl IntoResponse {
    match auth_session.user {
        Some(user) => {
            info!("get /prefs : {}", &user.name);
            Html(html_render::prefs(&user)).into_response()
        }
        None => unauthorized_response().into_response(),
    }
}

// TODO call add_library fn...
// TODO use struct, like new_user_handler()
async fn new_library_handler(auth_session: AuthSession, path: String) -> impl IntoResponse {
    match auth_session.user {
        Some(user) => {
            // only admin
            if user.role == Role::Admin {
                // retrieve path from body
                let path = path.split('=').next_back().unwrap_or("");
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
                Html(html_render::simple_message(
                    &format!(
                        "new library added, path :  {}<br /><a href=\"/admin\">return</a>",
                        decoded_path
                    ),
                    Some("/admin"),
                ))
                .into_response()
            } else {
                unauthorized_response().into_response()
            }
        }
        None => unauthorized_response().into_response(),
    }
}

/// use argon2 lib to hash password (stronger than bcrypt)
fn hash_password(plain_text_password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hashed_password = match argon2.hash_password(plain_text_password.as_bytes(), &salt) {
        Ok(hashed_password) => Ok(hashed_password.to_string()),
        Err(e) => {
            let msg = format!("unable to hash password : {e}");
            error!("{msg}");
            Err(msg)
        }
    };
    hashed_password
}

#[derive(Deserialize)]
struct FormUser {
    name: String,
    password: String,
    is_admin: Option<String>,
}
async fn new_user_handler(
    auth_session: AuthSession,
    Form(body): Form<FormUser>,
) -> impl IntoResponse {
    match auth_session.user {
        Some(user) => {
            if user.role == Role::Admin {
                // TODO check if name already exists
                match hash_password(&body.password) {
                    Ok(hashed_password) => {
                        let new_user = User {
                            name: body.name,
                            password_hash: hashed_password,
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
                                Html(html_render::simple_message("user created", Some("/admin")))
                                    .into_response()
                            }
                            Err(_) => error_handler().into_response(),
                        }
                    }
                    Err(_) => Html(html_render::simple_message(
                        "unable to add new user, see logs",
                        Some("/admin"),
                    ))
                    .into_response(),
                }
            } else {
                Html(html_render::simple_message(
                    "your are not allowed to create users",
                    Some("/"),
                ))
                .into_response()
            }
        }
        None => unauthorized_response().into_response(),
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
    auth_session: AuthSession,
    Path(user_id): Path<String>,
    Form(body): Form<FormUpdateUser>,
) -> impl IntoResponse {
    match auth_session.user {
        Some(user) => {
            if user.role == Role::Admin {
                match hash_password(&body.password) {
                    Ok(hashed_password) => match sqlite::create_sqlite_pool().await {
                        Ok(conn) => {
                            let check_user = sqlite::get_user(None, Some(&user_id), &conn).await;
                            if check_user.is_empty() {
                                Html(html_render::simple_message(
                                    &format!("user id {} does not exists", &user_id),
                                    Some("/admin"),
                                ))
                                .into_response()
                            } else {
                                let mut user_to_update = check_user.first().unwrap().to_owned();
                                if body.delete.is_some() && user_to_update.id != 1 {
                                    sqlite::delete_user(&user_to_update, &conn).await;
                                    Html(html_render::simple_message(
                                        &format!("user {} deleted", &user_to_update.name),
                                        Some("/admin"),
                                    ))
                                    .into_response()
                                } else if body.update.is_some() {
                                    if !body.password.is_empty() {
                                        user_to_update.password_hash = hashed_password;
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
                                        &format!("user {} updated", &user_to_update.name),
                                        Some("/admin"),
                                    ))
                                    .into_response()
                                } else {
                                    Html(html_render::simple_message(
                                        "you can't delete admin account",
                                        Some("/admin"),
                                    ))
                                    .into_response()
                                }
                            }
                        }
                        Err(_) => error_handler().into_response(),
                    },
                    Err(_) => Html(html_render::simple_message(
                        "unable to hash password",
                        Some("/"),
                    ))
                    .into_response(),
                }
            } else {
                Html(html_render::simple_message(
                    "your are not allowed to modify users",
                    Some("/"),
                ))
                .into_response()
            }
        }
        None => unauthorized_response().into_response(),
    }
}

// TODO admin only and call delete_library fn...
async fn admin_library_handler(
    auth_session: AuthSession,
    Path(library_id): Path<String>,
    // TODO use struct, like new_user_handler()
    body: String,
) -> impl IntoResponse {
    match auth_session.user {
        Some(user) => {
            // only admin
            if user.role == Role::Admin {
                let vec_body: Vec<&str> = body.split('=').collect();
                let option = vec_body.first().unwrap_or(&"").to_string();
                let _value = vec_body.last().unwrap_or(&"").to_string();
                match sqlite::create_sqlite_pool().await {
                    Ok(conn) => {
                        match option.as_str() {
                    "delete" => {
                        // TODO handle library.first() like for `full_rescan`
                        let library = sqlite::get_library(None, Some(&library_id), &conn).await;
                        info!("user [{}] asked for delete library [{}]", &user.name, &library[0].name);
                        sqlite::delete_library_from_id(&library, &conn).await;
                        // TODO delete in tables `covers`, `directories` and `reading`
                        sqlite::delete_files_from_library(&library, &conn).await;
                        info!("library [{}] deleted", &library[0].name);
                        Html(html_render::simple_message(
                            &format!("delete lib id = {}", &library[0].name),
                            Some("/admin"),
                        ))
                        .into_response()
                    }
                    "full_rescan" => {
                        match sqlite::get_library(None, Some(&library_id), &conn)
                            .await
                            .first()
                        {
                            Some(library) => {
                                info!("user [{}] asked for a full rescan of library [{}]", &user.name, &library.name);
                                scanner::launch_scan(library, &conn).await.ok();
                                Html(html_render::simple_message(
                                    &format!("library {} scanned (<a href=\"/admin\">return to admin panel</a>)", &library.name),
                                    Some("/admin"),
                                ))
                                .into_response()
                            }
                            None => {
                                Html(html_render::simple_message(
                                    "unable to find library in database",
                                    Some("/admin"),
                                ))
                                .into_response()
                            }
                        }
                    }
                    "covers" => Html(format!("TODO : lib id = {library_id}, covers flag toggle (<a href=\"/admin\">return to admin panel</a>)"))
                        .into_response(),
                    _ => error_handler().into_response(),
                }
                    }
                    Err(_) => error_handler().into_response(),
                }
            } else {
                unauthorized_response().into_response()
            }
        }
        None => unauthorized_response().into_response(),
    }
}

// TODO better display, and redirect to `/` after 3s
fn unauthorized_response() -> Html<String> {
    Html(String::from("You are not allowed to see this page"))
}

// TODO better display, and redirect to `/` after 3s
fn authent_error() -> String {
    String::from("Autentication error")
}

async fn library_handler(
    auth_session: AuthSession,
    path: Option<Path<String>>,
) -> impl IntoResponse {
    match auth_session.user {
        Some(user) => {
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
                            // TODO move this in `sqlite` mod
                            match sqlx::query_as("SELECT * FROM libraries;")
                                .fetch_all(&conn)
                                .await
                            {
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
                                file_count: Some(library.file_count),
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
                                let mut vec_splitted_path: VecDeque<&str> =
                                    path.split('/').collect();
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
                        let libraries_vec =
                            sqlite::get_library(Some(&library_name), None, &conn).await;
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
                                    sqlite::get_flag_status("bookmark", user.id, &file.id, &conn)
                                        .await;
                                let read_status = sqlite::get_flag_status(
                                    "read_status",
                                    user.id,
                                    &file.id,
                                    &conn,
                                )
                                .await;
                                files_list_with_status.push((file, bookmark_status, read_status));
                            }
                            files_list_with_status
                        };
                        files_list_with_status.sort();

                        let mut directories_list: Vec<DirectoryInfo> = {
                            info!("get /library{} : {}", &sub_path, &user.name);
                            // TODO set limit in conf
                            let directories_list: Vec<DirectoryInfo> = match sqlx::query_as(
                                "SELECT * FROM directories WHERE parent_path = ?;",
                            )
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
        None => unauthorized_response(),
    }
}

async fn get_root(auth_session: AuthSession) -> impl IntoResponse {
    match auth_session.user {
        Some(_) => {
            debug!("GET /, user found, redirect to /library");
            axum::response::Redirect::permanent("/library").into_response()
        }
        None => {
            debug!("GET /, no user found, login form");
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, "text/html"),
                    (header::VARY, "Accept-Encoding"),
                ],
                Html(html_render::login_form()),
            )
                .into_response()
        }
    }
}

// TODO factorize...
fn get_svg(svg_filename: &str) -> impl IntoResponse {
    let image = fs::read(svg_filename);
    match image {
        Ok(image) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "image/svg+xml"),
                (header::CONTENT_ENCODING, "gzip"),
                (header::VARY, "Accept-Encoding"),
                (header::CACHE_CONTROL, "public, max-age=604800"),
            ],
            image,
        )
            .into_response(),
        Err(_) => {
            error!("{svg_filename} not found");
            // TODO true 404
            (StatusCode::NOT_FOUND, "image not found").into_response()
        }
    }
}
fn get_png(png_filename: &str) -> impl IntoResponse {
    let image = fs::read(png_filename);
    match image {
        Ok(image) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "image/png"),
                (header::VARY, "Accept-Encoding"),
                (header::CACHE_CONTROL, "public, max-age=604800"),
            ],
            image,
        )
            .into_response(),
        Err(_) => {
            error!("{png_filename} not found");
            // TODO true 404
            (StatusCode::NOT_FOUND, "image not found").into_response()
        }
    }
}
async fn get_root_file(Path(path): Path<String>) -> impl IntoResponse {
    info!("get /{}", &path);
    match path.as_str() {
        "favicon.svgz" => get_svg("images/favicon.svgz").into_response(),
        "favicon-96x96.png" => get_png("images/favicon-96x96.png").into_response(),
        "favicon.ico" => {
            let image = fs::read("images/favicon.ico");
            match image {
                Ok(image) => (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "image/vnd.microsoft.icon"),
                        (header::VARY, "Accept-Encoding"),
                        (header::CACHE_CONTROL, "public, max-age=604800"),
                    ],
                    image,
                )
                    .into_response(),
                Err(_) => {
                    error!("{path} not found");
                    // TODO true 404
                    (StatusCode::NOT_FOUND, "image not found").into_response()
                }
            }
        }
        "apple-touch-icon.png" => get_png("images/apple-touch-icon.png").into_response(),
        "web-app-manifest-192x192.png" => {
            get_png("images/web-app-manifest-192x192.png").into_response()
        }
        "web-app-manifest-512x512.png" => {
            get_png("images/web-app-manifest-512x512.png").into_response()
        }
        "site.webmanifest" => {
            let webmanifest = include_bytes!("../site.webmanifest");
            let webmanifest = match std::str::from_utf8(webmanifest) {
                Ok(webmanifest) => webmanifest.to_string(),
                Err(_) => String::from(""),
            };
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, "application/manifest+json"),
                    (header::CACHE_CONTROL, "public, max-age=604800"),
                ],
                webmanifest,
            )
                .into_response()
        }
        _ => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

/// create css from binary if not found on disk
// TODO add a clap option to specify css directory
// TODO use struct ?
fn create_css() -> String {
    // original css file
    let eloran_css_original = include_bytes!("../css/eloran.css");
    let mut eloran_css = match std::str::from_utf8(eloran_css_original) {
        Ok(eloran_css) => eloran_css.to_string(),
        Err(_) => String::from(""),
    };
    // if custom css exists, use them
    let css_dir = std::path::Path::new("custom_css");
    if css_dir.is_dir() {
        let css_files = css_dir.read_dir().unwrap();
        for file in css_files.flatten() {
            let filename = file.file_name();
            let filename = filename.to_str().unwrap();
            if filename.contains("eloran.css") {
                eloran_css = fs::read_to_string(file.path()).unwrap();
            } else {
                warn!(
                    "css file must be named eloran.css, file [{}] will be ignored",
                    filename
                );
            }
        }
    }
    eloran_css
}

/// serve css (custom file can be loaded)
async fn get_css(State(css): State<String>, Path(path): Path<String>) -> impl IntoResponse {
    info!("get /css/{}", &path);
    // return css if found
    match path.as_str() {
        "eloran.css" => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/css"),
                (header::CACHE_CONTROL, "public, max-age=604800"),
            ],
            css,
        )
            .into_response(),
        // useless : the html headers uses only eloran.css but perhaps in the future...
        _ => {
            let css_file_content = fs::read_to_string(format!("css/{path}"));
            match css_file_content {
                Ok(css) => {
                    (StatusCode::OK, [(header::CONTENT_TYPE, "text/css")], css).into_response()
                }
                Err(_) => {
                    error!("css {path} not found");
                    // TODO true 404 page ?
                    (StatusCode::NOT_FOUND, "css not found").into_response()
                }
            }
        }
    }
}

/// serve fonts
/// TODO see https://stackoverflow.com/questions/75065364/how-to-include-font-file-assets-folder-to-rust-binary ?
async fn get_fonts(Path(path): Path<String>) -> impl IntoResponse {
    info!("get /fonts/{}", &path);

    // return font if found
    match path.as_str() {
        "Exo-VariableFont_wght.ttf" => {
            match fs::read("fonts/Exo-VariableFont_wght.ttf") {
                Ok(exo_font) => (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "font/ttf"),
                        // TODO fix cache !!!
                        (header::CACHE_CONTROL, "public, max-age=604800"),
                    ],
                    exo_font,
                )
                    .into_response(),
                Err(_) => {
                    error!("unable to load exo font");
                    // TODO true 404 page ?
                    (StatusCode::NOT_FOUND, "font not found").into_response()
                }
            }
        }
        _ => {
            error!("font {path} not found");
            // TODO true 404 page ?
            (StatusCode::NOT_FOUND, "font not found").into_response()
        }
    }
}

async fn get_images(Path(path): Path<String>) -> impl IntoResponse {
    info!("get /images/{}", &path);
    // TODO include_bytes pour la base ? (cf monit-agregator)
    // read_to_string if svg instead of svgz
    // https://developer.mozilla.org/en-US/docs/Web/SVG/Tutorial/Getting_Started#a_word_on_web_servers_for_.svgz_files
    let image_file_content = fs::read(format!("images/{path}"));
    // TODO tests content pour 200 ?
    match image_file_content {
        Ok(image) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "image/svg+xml"),
                (header::CONTENT_ENCODING, "gzip"),
                (header::VARY, "Accept-Encoding"),
                (header::CACHE_CONTROL, "public, max-age=604800"),
            ],
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

// 🔥🔥 🔥 🔥 🔥 🔥  AXUMLOGIN🔥 🔥 🔥 🔥 🔥 🔥
// #[derive(Debug, Clone)]
use async_trait::async_trait;
use axum_login::{AuthUser, AuthnBackend, UserId};
use password_auth::verify_password;
use sqlx::SqlitePool;
impl AuthUser for User {
    type Id = i64;
    fn id(&self) -> Self::Id {
        self.id
    }
    fn session_auth_hash(&self) -> &[u8] {
        self.password_hash.as_bytes() // We use the password hash as the auth
                                      // hash--what this means
                                      // is when the user changes their password the
                                      // auth session becomes invalid.
    }
}
// Serialize is for testing with axum-test
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
    pub next: Option<String>,
}
#[derive(Debug, Clone)]
pub struct Backend {
    db: SqlitePool,
}
impl Backend {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }
}
#[async_trait]
impl AuthnBackend for Backend {
    type User = User;
    type Credentials = Credentials;
    type Error = sqlx::Error;
    async fn authenticate(
        &self,
        creds: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        let user: Option<Self::User> = sqlx::query_as("select * from users where name = ? ")
            .bind(creds.username)
            .fetch_optional(&self.db)
            .await?;
        Ok(user.filter(|user| {
            verify_password(creds.password, &user.password_hash)
                .ok()
                .is_some() // We're using password-based authentication--this
                           // works by comparing our form input with an argon2
                           // password hash.
        }))
    }
    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        let user = sqlx::query_as("select * from users where id = ?")
            .bind(user_id)
            .fetch_optional(&self.db)
            .await?;
        Ok(user)
    }
}

pub type AuthSession = axum_login::AuthSession<Backend>;

async fn create_router() -> Router {
    match sqlite::create_sqlite_pool().await {
        Ok(pool) => {
            // Session layer
            // see example : https://github.com/maxcountryman/axum-login/blob/main/examples
            // This uses `tower-sessions` to establish a layer that will provide the session
            // as a request extension.
            let session_store = MemoryStore::default(); // TODO do not use MemoryStore in prod ?
            let session_layer = SessionManagerLayer::new(session_store)
                .with_secure(false)
                .with_expiry(Expiry::OnInactivity(Duration::days(1)));
            // Auth service
            // This combines the session layer with our backend to establish the auth
            // service which will provide the auth session as a request extension.
            let backend = Backend::new(pool.clone());
            let auth_service = AuthManagerLayerBuilder::new(backend, session_layer).build();

            // custom css handler, will be passed to the css route
            let css = create_css();

            // Router creation
            Router::new()
                // 🔒🔒🔒 ADMIN PROTECTED 🔒🔒🔒
                .route("/admin", get(admin_handler))
                .route("/admin/library/{library_id}", post(admin_library_handler))
                .route("/admin/library/new", post(new_library_handler))
                .route("/admin/user/{user_id}", post(change_user_handler))
                .route("/admin/user/new", post(new_user_handler))
                // TODO PROTECT HERE : add a layer (Role::Admin) if possible
                // 🔒🔒🔒 PROTECTED 🔒🔒🔒
                .route("/prefs", get(prefs_handler))
                .route("/library", get(library_handler))
                .route("/library/{*path}", get(library_handler))
                .route("/toggle/{flag}/{id}", get(flag_handler))
                .route("/bookmarks", get(bookmarks_handler))
                .route("/reading", get(reading_handler))
                .route("/search", post(search_handler))
                .route("/download/{file_id}", get(download_handler))
                .route("/read/{file_id}/{page}", get(reader_handler))
                .route(
                    "/comic_page/{file_id}/{page}/{size}",
                    get(comic_page_handler),
                )
                .route("/infos/{file_id}", get(infos_handler))
                .route("/cover/{file_id}", get(cover_handler))
                .route_layer(login_required!(Backend, login_url = "/"))
                // TODO PROTECT HERE : add a layer (Role::User) if possible
                // 🔥🔥🔥 UNPROTECTED 🔥🔥🔥
                .route("/", get(get_root))
                .route("/{path}", get(get_root_file))
                .route("/css/{*path}", get(get_css))
                .route("/fonts/{*path}", get(get_fonts))
                .with_state(css)
                .with_state(pool)
                .route("/images/{*path}", get(get_images)) // ⚠️  UI images, not covers
                .route("/login", post(login_handler))
                .route("/logout", get(logout_handler))
                // .fallback(fallback) // TODO useless ?
                // ---
                // layers for redirect when not logged
                // see https://github.com/maxcountryman/axum-login/issues/22#issuecomment-1345403733
                .layer(auth_service)
                .layer(
                    ServiceBuilder::new()
                        // .layer(session_layer)
                        // .layer(auth_layer)
                        .map_response(|response: Response| {
                            if response.status() == StatusCode::UNAUTHORIZED {
                                Redirect::to("/").into_response()
                            } else {
                                response
                            }
                        }),
                )
        }
        // TODO true error handling and template page
        Err(_) => {
            error!("unable to connect to database, exiting");
            process::exit(1);
        }
    }
}

pub async fn start_http_server(bind: &str) -> Result<(), String> {
    info!("start http server on {}", bind);
    // TODO handle error, and default value
    let router = create_router();

    // TODO trim trailing slash
    // see https://docs.rs/tower-http/latest/tower_http/normalize_path/struct.NormalizePathLayer.html?search=trim_trailing_slash#method.trim_trailing_slash
    // and
    // https://stackoverflow.com/questions/75355826/route-paths-with-or-without-of-trailing-slashes-in-rust-axum

    let listener = tokio::net::TcpListener::bind(bind).await.unwrap();
    axum::serve(listener, router.await.into_make_service())
        .await
        .expect("unable to bind http server");
    // TODO check if server started
    // axum::Server::bind(&bind)
    //     .serve(router.await.into_make_service())
    //     .await
    //     .expect("unable to bind http server");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use sqlx::{migrate::MigrateDatabase, Sqlite};

    #[tokio::test]
    async fn test_favicon() {
        let router = create_router().await;
        let client = TestServer::new(router).expect("new TestServer");
        let favicon = client.get("/favicon.ico").await;
        assert_eq!(favicon.status_code(), StatusCode::OK);
        let touchicon = client.get("/apple-touch-icon.png").await;
        assert_eq!(touchicon.status_code(), StatusCode::OK);
        let manifest = client.get("/site.webmanifest").await;
        assert_eq!(manifest.status_code(), StatusCode::OK);
        let largeicon = client.get("/web-app-manifest-192x192.png").await;
        assert_eq!(largeicon.status_code(), StatusCode::OK);
        // test 404
        client
            .get("/css/not_found")
            .expect_failure()
            .await
            .assert_status_not_found();
    }
    #[tokio::test]
    async fn test_login_logout() {
        // init db
        let _ = sqlite::init_database().await;
        sqlite::init_default_users().await;
        // create router
        let router = create_router();

        // root without auth
        let mut client = TestServer::new(router.await).expect("new TestServer");
        client.save_cookies();
        client.expect_success();

        let res = client.get("/").await;
        assert_eq!(res.status_code(), StatusCode::OK);
        insta::assert_yaml_snapshot!(res.text());

        // login
        let cred = Credentials {
            username: "admin".to_string(),
            password: "admin".to_string(),
            next: None,
        };
        let res = client
            .post("/login")
            // panic if form is not deserializable...
            .form(&cred)
            .expect_failure()
            .await;
        res.assert_status_see_other();
        res.assert_contains_header("set-cookie");
        insta::assert_yaml_snapshot!(res.text());

        // root with auth
        let res = client.get("/").expect_failure().await;
        assert_eq!(res.status_code(), StatusCode::PERMANENT_REDIRECT);
        let res = client.get("/library").await;
        assert_eq!(res.status_code(), StatusCode::OK);
        insta::assert_yaml_snapshot!(res.text());

        // logout
        let res = client.get("/logout").await;
        assert_eq!(res.status_code(), StatusCode::OK);
        insta::assert_yaml_snapshot!(res.text());

        // root without auth
        let res = client.get("/").await;
        assert_eq!(res.status_code(), StatusCode::OK);
        insta::assert_yaml_snapshot!(res.text());

        // css error
        client
            .get("/css/not_found")
            .expect_failure()
            .await
            .assert_status_not_found();

        client
            .get("/css/eloran.css")
            .await
            .assert_header("content-type", "text/css");

        // delete database
        let _ = Sqlite::drop_database(crate::DB_URL).await;
    }
}
