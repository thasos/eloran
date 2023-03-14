mod html_render;
mod http_server;
mod reader;
mod scanner;
mod sqlite;

#[macro_use]
extern crate log;
#[macro_use]
extern crate horrorshow;
use std::io::Error;
use std::path::Path;
use std::time::Duration;

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
        // TODO lib path in conf, need more checks of library ?
        let library_path = Path::new("library_test");
        // let library_path = Path::new("/home/thasos/books");
        // let library_path = Path::new("/palanthas/bd/bd");
        info!(
            "start scanner routine on library {}",
            library_path.to_string_lossy()
        );
        // TODO true error handling
        let sleep_time = Duration::from_secs(35);
        scanner::scan_routine(library_path, sleep_time).await;
    });

    // TODO true error handling
    debug!("try to start http server");
    http_server::start_http_server().await?;

    Ok(())
}
