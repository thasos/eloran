mod html_render;
mod http_server;

#[macro_use]
extern crate log;
#[macro_use]
extern crate horrorshow;
use std::io::Error;

fn main() -> Result<(), Error> {
    env_logger::init();
    info!("starting up");
    http_server::start_http_server()?;
    Ok(())
}
