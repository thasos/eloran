use clap::{arg, ArgAction, ArgMatches, Command};
use serde::Serialize;
use std::env;

#[derive(Serialize)]
pub struct Conf {
    pub bind: String,
    pub library_path: Option<Vec<String>>,
}
impl Conf {
    pub fn default() -> Conf {
        Conf {
            bind: format!("{DEFAULT_IP}:{DEFAULT_PORT}"),
            library_path: Some(Vec::with_capacity(0)),
        }
    }
}

const DEFAULT_IP: &str = "0.0.0.0";
const DEFAULT_PORT: &str = "3200";

/// create configuration
/// priority list : command line > environment variables (todo)
pub fn init_conf() -> Conf {
    const CARGO_PKG_VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");
    // parse args
    let clap_params = clap_args(CARGO_PKG_VERSION);
    let mut logbuilder = env_logger::Builder::from_default_env();
    logbuilder.target(env_logger::Target::Stdout);
    // debug mode
    if clap_params.get_flag("verbose") {
        logbuilder.filter_level(log::LevelFilter::Debug);
        if env::var("RUST_LOG").is_err() {
            // crates pdf and hypper are not too verbose
            logbuilder.filter(Some("sqlx::"), log::LevelFilter::Off);
            logbuilder.filter(Some("hyper::"), log::LevelFilter::Off);
        }
    } else if env::var("RUST_LOG").is_err() {
        logbuilder.filter_level(log::LevelFilter::Info);
        // crates pdf and hypper are not too verbose
        logbuilder.filter(Some("sqlx::"), log::LevelFilter::Off);
        logbuilder.filter(Some("hyper::"), log::LevelFilter::Off);
    }
    match logbuilder.try_init() {
        Ok(_) => (),
        Err(_) => eprintln!("unable to initiate pretty logging"),
    };

    info!(
        "starting up version={}",
        CARGO_PKG_VERSION.unwrap_or("version not found")
    );

    // library_path
    let library_path = match clap_params.get_many::<String>("library_path") {
        Some(library_path_from_cmd) => Some(
            library_path_from_cmd
                .cloned()
                .collect::<Vec<_>>()
                // delete last char if it's `/`
                .iter()
                .map(|p| p.trim_end_matches('/').to_string())
                .collect(),
        ),
        None => Conf::default().library_path,
    };

    // bind ip
    let ip = clap_params
        .get_one::<String>("ip")
        .expect("required")
        .to_owned();
    let port = clap_params
        .get_one::<String>("port")
        .expect("required")
        .to_owned();
    let bind = format!("{}:{}", ip, port);

    // return conf
    Conf { bind, library_path }
}

/// parse commande line arguments
pub fn clap_args(_version: Option<&str>) -> ArgMatches {
    Command::new("Eloran")
        // TODO use version const
        // .version("666")
        .author("Thasos Kanan <@thasos@framapiaf.org>")
        .about("💬 Comics and 📖 Ebook web library written in rust 🦀🚀")
        .arg(arg!(--ip <VALUE>).short('i').default_value(DEFAULT_IP))
        .arg(arg!(--port <VALUE>).short('p').default_value(DEFAULT_PORT))
        .arg(
            arg!(--library_path <VALUE>)
                .short('l')
                .num_args(1)
                .action(ArgAction::Append),
        )
        .arg(
            arg!(--verbose)
                .short('v')
                .long("verbose")
                .action(ArgAction::SetTrue),
        )
        .get_matches()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_conf() {
        insta::assert_yaml_snapshot!(init_conf())
    }
}
