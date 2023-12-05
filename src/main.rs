mod conf;
mod html_render;
mod http_server;
mod reader;
mod scanner;
mod sqlite;

#[macro_use]
extern crate log;
#[macro_use]
extern crate horrorshow;
use std::time::Duration;

use crate::conf::init_conf;

const DB_URL: &str = "sqlite://sqlite/eloran.db";

#[tokio::main]
async fn main() -> Result<(), String> {
    // conf
    let conf: conf::Conf = init_conf();

    // databases
    sqlite::init_database().await?;
    // TODO remove dbg users when install page is done
    sqlite::init_default_users().await;
    if conf.library_path.is_some() {
        sqlite::create_library_path(conf.library_path.unwrap()).await;
    }

    // start routines
    // scan the library files and add them in database
    tokio::spawn(async {
        info!("start scanner routine");
        let sleep_time = Duration::from_secs(300);
        scanner::scan_routine(sleep_time).await;
    });
    // retrieve files list from database and extract covers and some metadatas
    tokio::spawn(async {
        info!("start extractor routine");
        // 100 files per 10 second
        let extraction_speed = 100;
        let sleep_time = Duration::from_secs(250);
        scanner::extraction_routine(extraction_speed, sleep_time).await;
    });

    // start web server
    http_server::start_http_server(&conf.bind).await?;

    Ok(())
}
