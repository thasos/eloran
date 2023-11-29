use clap::{arg, ArgAction, ArgMatches, Command};
use config::Config;
use serde::Serialize;
use std::env;

#[derive(Serialize)]
pub struct Conf {
    pub bind: String,
    pub library_path: Option<Vec<String>>,
}

const DEFAULT_IP: &str = "0.0.0.0";
const DEFAULT_PORT: &str = "3200";
const DEFAULT_CONFIG_FILE: &str = "settings.yaml";

/// create configuration
/// priority list : command line > environment variables (todo) > config file
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
        }
    } else if env::var("RUST_LOG").is_err() {
        logbuilder.filter_level(log::LevelFilter::Info);
        // crates pdf and hypper are not too verbose
        logbuilder.filter(Some("sqlx::"), log::LevelFilter::Off);
    }
    match logbuilder.try_init() {
        Ok(_) => (),
        Err(_) => eprintln!("unable to initiate pretty logging"),
    };

    info!(
        "starting up version={}",
        CARGO_PKG_VERSION.unwrap_or("version not found")
    );

    // config file
    let config_file_path = clap_params
        .get_one::<String>("config")
        .expect("required")
        .to_owned();

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
        None => {
            if let Ok(settings) = Config::builder()
                .add_source(config::File::with_name(&config_file_path))
                .build()
            {
                if let Ok(library_path_from_conf) = settings.get_array("library_path") {
                    let library_path_list: Vec<String> = library_path_from_conf
                        .iter()
                        .map(|v| v.to_string().trim_end_matches('/').to_string())
                        .collect();
                    Some(library_path_list)
                } else {
                    warn!("unable to get library_path from command line");
                    None
                }
            } else {
                // no library path from command line or conf
                None
            }
        }
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
    let mut bind = format!("{}:{}", ip, port);
    if bind == format!("{}:{}", DEFAULT_IP, DEFAULT_PORT) {
        if let Ok(settings) = Config::builder()
            // TODO add ELORAN_ip and ELORAN_port support ?
            .add_source(config::File::with_name(&config_file_path))
            .add_source(config::Environment::with_prefix("ELORAN"))
            .build()
        {
            bind = settings.get_string("bind").unwrap();
        };
    }

    // return conf
    Conf { bind, library_path }
}

/// parse commande line arguments
pub fn clap_args(_version: Option<&str>) -> ArgMatches {
    Command::new("Eloran")
        // TODO use version const
        // .version("666")
        .author("Thasos Kanan <thasos@thasmanie.fr>")
        .about("TODO")
        .arg(arg!(--ip <VALUE>).short('i').default_value(DEFAULT_IP))
        .arg(arg!(--port <VALUE>).short('p').default_value(DEFAULT_PORT))
        .arg(
            arg!(--library_path <VALUE>)
                .short('l')
                .num_args(1)
                .action(ArgAction::Append),
        )
        .arg(
            arg!(--config <VALUE>)
                .short('c')
                .default_value(DEFAULT_CONFIG_FILE),
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
