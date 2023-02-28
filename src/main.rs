mod html_render;
mod http_server;
mod scanner;
mod sqlite;

#[macro_use]
extern crate log;
#[macro_use]
extern crate horrorshow;
use std::io::Error;

const DB_URL: &str = "sqlite://sqlite/eloran.db";

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();
    const CARGO_PKG_VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");
    info!(
        "starting up version={}",
        CARGO_PKG_VERSION.unwrap_or("version not found")
    );

    // TODO use this const...
    sqlite::init_database().await;
    // TODO only if needed...
    sqlite::init_users(DB_URL).await;

    tokio::spawn(async {
        // TODO true error handling
        scanner::scan_routine().await;
    });

    // TODO true error handling
    debug!("try to start http server");
    http_server::start_http_server().await?;

    Ok(())
}
