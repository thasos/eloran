use crate::html_render::{self, login_ok};
use crate::scanner::FileInfo;
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
    let mut conn = sqlite::create_sqlite_connection().await;

    // get user from db
    // TODO hash password
    Html(
        match sqlx::query_as(&format!(
            "SELECT * FROM users WHERE name = '{}' AND password_hash = '{}'",
            &username, &password
        ))
        .fetch_one(&mut conn)
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
    let toto = match &auth.current_user {
        Some(user) => {
            debug!("user found, logout");
            Html(html_render::logout(user))
        }
        None => {
            warn!("no user found, can't logout !");
            Html("Err".to_string())
        }
    };
    toto
}

async fn library_handler(Extension(user): Extension<User>) -> impl IntoResponse {
    info!("get /library : {user:?}");

    // retrieve books list
    let mut conn = sqlite::create_sqlite_connection().await;
    // TODO set limit in conf
    let publication_list: Vec<FileInfo> = match sqlx::query_as("SELECT * FROM library LIMIT 20")
        .fetch_all(&mut conn)
        .await
    {
        Ok(publication_list) => publication_list,
        Err(e) => {
            warn!("empty library : {}", e);
            let empty_list: Vec<FileInfo> = Vec::new();
            empty_list
        }
    };
    Html(html_render::library(&user, publication_list))
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
    let css_file_content = fs::read_to_string(format!("src/css/{}", path));
    // TODO tests content pour 200 ?
    match css_file_content {
        Ok(css) => (StatusCode::OK, [(header::CONTENT_TYPE, "text/css")], css).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "css not found").into_response(),
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
        .route("/library", get(library_handler))
        .route_layer(RequireAuthorizationLayer::<User, Role>::login_with_role(
            Role::User..,
        ))
        // 🔥 UNPROTECTED 🔥
        .route("/", get(get_root))
        .route("/css/*path", get(get_css))
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

    #[tokio::test]
    async fn test_login_logout() {
        // headers
        let headers = "<!DOCTYPE html><html><head><title>Eloran</title><meta charset=\"UTF-8\"><meta name=\"viewport\" content=\"width=device-width\"><link rel=\"stylesheet\" href=\"css/w3.css\"><link rel=\"stylesheet\" href=\"css/w3-theme-dark-grey.css\"><meta http-equiv=\"Cache-Control\" content=\"no-cache, no-store, must-revalidate\"><meta http-equiv=\"Pragma\" content=\"no-cache\"><meta http-equiv=\"Expires\" content=\"0\"></head><body class=\"w3-theme-dark\">";
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
    }
}

#[test]
fn parse_user_password_test() {
    let body = String::from("user=myuser&password=mypass");
    let (user, password) = parse_credentials(&body);
    assert_eq!(user, String::from("myuser"));
    assert_eq!(password, String::from("mypass"));
}
