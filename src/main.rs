#![forbid(unsafe_code)]
mod conf;
mod html_render;
mod http_server;
mod reader;
mod scanner;
mod sqlite;

use crate::conf::init_conf;
use log::{error, info};
use std::{env, time::Duration};

const DB_URL: &str = "sqlite://sqlite/eloran.db";

#[tokio::main]
async fn main() -> Result<(), String> {
    // conf
    let args: Vec<String> = env::args().collect();
    let conf: conf::Conf = init_conf(args);

    // databases
    if let Err(e) = sqlite::init_database().await {
        error!("Unable to create database");
        return Err(e);
    }
    // TODO remove defaults users when install page is done
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
        let sleep_time = Duration::from_secs(600);
        scanner::extraction_routine(extraction_speed, sleep_time).await;
    });

    // start web server
    if let Err(e) = http_server::start_http_server(&conf.bind).await {
        error!("Unable to start http server");
        return Err(e);
    }

    Ok(())
}
