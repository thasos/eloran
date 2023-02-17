use crate::html_render;
use axum::response::{Html, IntoResponse};
use axum::{
    routing::{get, post},
    Extension, Router,
};
use axum_login::{
    axum_sessions::{async_session::MemoryStore, SessionLayer},
    secrecy::SecretVec,
    AuthLayer, AuthUser, RequireAuthorizationLayer, SqliteStore,
};
use html_render::Page;
use rand::Rng;
use sqlx::sqlite::SqlitePoolOptions;
use std::io::Error;
use std::process;

// User Struct
// TODO virer Default ?
#[derive(Debug, Default, Clone, sqlx::FromRow)]
struct User {
    id: i64,
    password_hash: String,
    name: String,
    role: Role,
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

async fn create_sqlite_pool() -> sqlx::pool::PoolConnection<sqlx::Sqlite> {
    let pool = SqlitePoolOptions::new()
        // TODO db path in conf
        .connect("sqlite/user_store.db")
        .await
        .unwrap();
    let conn = pool.acquire().await.unwrap();
    conn
}

async fn login_handler(mut auth: AuthContext, body: String) -> impl IntoResponse {
    info!("get /login : {}", &body);
    let (username, password) = parse_credentials(&body);

    // connect to db
    let mut conn = create_sqlite_pool().await;

    // get user from db
    // TODO hash password
    let user: User = match sqlx::query_as(&format!(
        "SELECT * FROM users WHERE name = '{}' AND password_hash = '{}'",
        &username, &password
    ))
    .fetch_one(&mut conn)
    .await
    {
        Ok(s) => s,
        Err(_) => {
            info!("{} not found", &username);
            // TODO pas d'exit...
            process::exit(1);
        }
    };
    // TODO if password match
    auth.login(&user).await.unwrap();

    // TODO : vraie page
    Html(format!(
        "Successfully logged in as: {}, role {:?}",
        user.name, user.role
    ))
}

async fn logout_handler(mut auth: AuthContext) -> impl IntoResponse {
    // dbg!("Logging out user: {}", &auth.current_user);
    // auth.logout().await;
    info!("get /logout : {:?}", &auth.current_user);
    auth.logout().await;
    Html(html_render::render({
        match &auth.current_user {
            Some(user) => (Page::Logout, Some(user.name.clone())),
            None => (Page::BiffTheUnderstudy, None),
        }
    }))
}

async fn library_handler(Extension(user): Extension<User>) -> impl IntoResponse {
    info!("get /library : {user:?}");
    Html(format!("Logged in as: {}, role {:?}", user.name, user.role))
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
    Html(html_render::render({
        match user {
            Some(user) => (Page::Root, Some(user.name)),
            None => (Page::Login, None),
        }
    }))
}

async fn create_router() -> Router {
    let secret = rand::thread_rng().gen::<[u8; 64]>();
    // TODO MemoryStore KO en prod
    let session_store = MemoryStore::new();
    // TODO cookies options (secure, ttl, ...) :
    // https://docs.rs/axum-sessions/0.4.1/axum_sessions/struct.SessionLayer.html#implementations
    let session_layer = SessionLayer::new(session_store, &secret).with_secure(false);
    // TODO use fn create_sqlite_pool
    let pool = SqlitePoolOptions::new()
        .connect("sqlite/user_store.db")
        .await
        .unwrap();
    let user_store = SqliteStore::<User, Role>::new(pool);
    let auth_layer = AuthLayer::new(user_store, &secret);

    Router::new()
        // 🔒 protected 🔒
        .route("/admin", get(admin_handler))
        // .route_layer(RequireAuthorizationLayer::<User>::login())
        .route_layer(RequireAuthorizationLayer::<User, Role>::login_with_role(
            Role::Admin..,
        ))
        .route("/library", get(library_handler))
        // .route_layer(RequireAuthorizationLayer::<User>::login())
        .route_layer(RequireAuthorizationLayer::<User, Role>::login_with_role(
            Role::User..,
        ))
        // 🔥 UNPROTECTED 🔥
        .route("/", get(get_root))
        .route("/login", post(login_handler))
        .route("/logout", get(logout_handler))
        .layer(auth_layer)
        .layer(session_layer)
}

#[tokio::main]
pub async fn start_http_server() -> Result<(), Error> {
    let router = create_router();

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
        // create router
        let router = create_router();
        // root without auth
        let client = TestClient::new(router.await);
        let res = client.get("/").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "<!DOCTYPE html><html><head><title>Eloran</title></head><body><h2 id=\"heading\">Welcome to Eloran</h2><p>Please login :</p><p><form action=\"/login\" method=\"post\"><input type=\"text\" name=\"user\" placeholder=\"username\" required><br><input type=\"password\" name=\"password\" placeholder=\"password\" required><br><input type=\"submit\" value=\"Login\"></form></p></body></html>");
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
            "Successfully logged in as: admin, role Admin"
        );
        // root with auth
        let res = client.get("/").header("Cookie", &cookie).send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "<!DOCTYPE html><html><head><title>Eloran</title></head><body><h2 id=\"heading\">Welcome to Eloran</h2><p>Logged in as: admin</p></body></html>");
        // logout
        let res = client.get("/logout").header("Cookie", &cookie).send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "<!DOCTYPE html><html><head><title>Eloran</title></head><body><h2 id=\"heading\">Welcome to Eloran</h2><p>Bye admin</p></body></html>");
        // root without auth
        let res = client.get("/").header("Cookie", &cookie).send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "<!DOCTYPE html><html><head><title>Eloran</title></head><body><h2 id=\"heading\">Welcome to Eloran</h2><p>Please login :</p><p><form action=\"/login\" method=\"post\"><input type=\"text\" name=\"user\" placeholder=\"username\" required><br><input type=\"password\" name=\"password\" placeholder=\"password\" required><br><input type=\"submit\" value=\"Login\"></form></p></body></html>");
    }
}

#[test]
fn parse_user_password_test() {
    let body = String::from("user=myuser&password=mypass");
    let (user, password) = parse_credentials(&body);
    assert_eq!(user, String::from("myuser"));
    assert_eq!(password, String::from("mypass"));
}
