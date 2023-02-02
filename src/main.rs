mod html_render;
mod http_server;

#[macro_use]
extern crate horrorshow;
use std::io::Error;

fn main() -> Result<(), Error> {
    http_server::start_http_server()?;
    Ok(())
}
