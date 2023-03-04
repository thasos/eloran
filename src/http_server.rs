use crate::html_render::{self, login_ok};
use crate::scanner::{DirectoryInfo, FileInfo};
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
use sqlx::SqlitePool;
use std::fs;
use std::io::Error;
use tower::ServiceBuilder;

// User Struct
// TODO virer Default ?
#[derive(Debug, Default, Clone, sqlx::FromRow)]
pub struct User {
    // TODO ulid ?
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

async fn login_handler(mut auth: AuthContext, body: String) -> impl IntoResponse {
    info!("get /login : {}", &body);
    let (username, password) = parse_credentials(&body);

    // connect to db
    let conn = SqlitePool::connect(crate::DB_URL).await.unwrap();

    // get user from db
    // TODO hash password
    Html(
        match sqlx::query_as(&format!(
            "SELECT * FROM users WHERE name = '{}' AND password_hash = '{}'",
            &username, &password
        ))
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
        },
    )
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

async fn reader_handler(
    Extension(user): Extension<User>,
    path: Option<Path<String>>,
) -> impl IntoResponse {
    let conn = SqlitePool::connect(crate::DB_URL).await.unwrap();
    let library_path: String = sqlite::get_library_path(&conn).await;
    let path = match path {
        Some(path) => format!("/{}", path.as_str()),
        None => String::new(),
    };
    let toto = format!("coucou : {} {} {}", user.name, path, library_path);
    Html(toto)
}

async fn library_handler(
    Extension(user): Extension<User>,
    path: Option<Path<String>>,
) -> impl IntoResponse {
    let conn = SqlitePool::connect(crate::DB_URL).await.unwrap();
    let library_path: String = sqlite::get_library_path(&conn).await;
    let path = match path {
        Some(path) => format!("/{}", path.as_str()),
        None => String::new(),
    };

    let files_list: Vec<FileInfo> = {
        info!("get /library{} : {}", path, user.name);
        // TODO set limit in conf
        let files_list: Vec<FileInfo> = match sqlx::query_as(&format!(
            "SELECT * FROM files WHERE parent_path = '{library_path}{}' LIMIT 20",
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
        files_list
    };

    let directories_list: Vec<DirectoryInfo> = {
        info!("get /library{} : {}", path, user.name);
        // TODO set limit in conf
        let directories_list: Vec<DirectoryInfo> = match sqlx::query_as(&format!(
            "SELECT * FROM directories WHERE parent_path = '{library_path}{}' LIMIT 20",
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

    Html(html_render::library(
        &user,
        path,
        directories_list,
        files_list,
        library_path,
    ))
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
            // [(header::CONTENT_TYPE, "image/svg+xml")],
            [(header::CONTENT_TYPE, "image/svg+xml")],
            [(header::CONTENT_ENCODING, "gzip")],
            [(header::VARY, "Accept-Encoding")],
            image,
        )
            .into_response(),
        Err(_) => {
            error!("images {path} not found");
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
    // TODO use fn create_sqlite_pool
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
        .route("/read/*path", get(reader_handler))
        .route_layer(RequireAuthorizationLayer::<User, Role>::login_with_role(
            Role::User..,
        ))
        // 🔥 UNPROTECTED 🔥
        .route("/", get(get_root))
        .route("/css/*path", get(get_css))
        .route("/images/*path", get(get_images))
        .route("/login", post(login_handler))
        .route("/logout", get(logout_handler))
        // layers for redirect when not logged
        // see https://github.com/maxcountryman/axum-login/issues/22#issuecomment-1345403733
        .layer(
            ServiceBuilder::new()
                .layer(session_layer)
                .layer(auth_layer)
                .map_response(|r: Response| {
                    if r.status() == StatusCode::UNAUTHORIZED {
                        Redirect::to("/").into_response()
                    } else {
                        r
                    }
                }),
        )
}

pub async fn start_http_server() -> Result<(), Error> {
    let router = create_router();

    // TODO check si server bien started
    info!("(FAKE) http server started on 0.0.0.0:3000 (FAKE)");
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
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
        let headers = "<!DOCTYPE html><html><head><title>Eloran</title><meta charset=\"UTF-8\"><meta name=\"viewport\" content=\"width=device-width\"><link rel=\"stylesheet\" href=\"/css/w3.css\"><link rel=\"stylesheet\" href=\"/css/gallery.css\"><link rel=\"stylesheet\" href=\"/css/w3-theme-dark-grey.css\"><meta http-equiv=\"Cache-Control\" content=\"no-cache, no-store, must-revalidate\"><meta http-equiv=\"Pragma\" content=\"no-cache\"><meta http-equiv=\"Expires\" content=\"0\"></head><body class=\"w3-theme-dark\">";
        // create router
        let router = create_router();
        // root without auth
        let client = TestClient::new(router.await);
        let res = client.get("/").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, format!("{}<h2 id=\"heading\">Welcome to Eloran</h2><p>Please login :</p><p><form action=\"/login\" method=\"post\"><input type=\"text\" name=\"user\" placeholder=\"username\" required><br><input type=\"password\" name=\"password\" placeholder=\"password\" required><br><input type=\"submit\" value=\"Login\"></form></p></body></html>", headers));
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
            format!("{}<h2 id=\"heading\">Welcome to Eloran</h2><p>Successfully logged in as: admin, role Admin</p><p><a href=\"/\">return home</a></p></body></html>", headers));
        // root with auth
        let res = client.get("/").header("Cookie", &cookie).send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, format!("{}<h2 id=\"heading\">Welcome to Eloran</h2><div id=\"menu\"><p>Logged in as: admin, role Admin</p><p><a href=\"/library\">library</a></p><p><a href=\"/prefs\">preferences</a></p><p><a href=\"/logout\">logout</a></p></div><div id=\"home-content\">content</div></body></html>", headers));
        // logout
        let res = client.get("/logout").header("Cookie", &cookie).send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, format!("{}<h2 id=\"heading\">Welcome to Eloran</h2><p>Bye admin</p><p><a href=\"/\">return home</a></p></body></html>", headers));
        // root without auth
        let res = client.get("/").header("Cookie", &cookie).send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, format!("{}<h2 id=\"heading\">Welcome to Eloran</h2><p>Please login :</p><p><form action=\"/login\" method=\"post\"><input type=\"text\" name=\"user\" placeholder=\"username\" required><br><input type=\"password\" name=\"password\" placeholder=\"password\" required><br><input type=\"submit\" value=\"Login\"></form></p></body></html>", headers));
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
