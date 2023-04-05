use clap::{arg, ArgAction, ArgMatches, Command};
use config::Config;

pub struct Conf {
    pub bind: String,
    pub library_path: String,
}

const DEFAULT_IP: &str = "0.0.0.0";
const DEFAULT_PORT: &str = "3200";
const DEFAULT_LIBRARY_PATH: &str = "library";
const DEFAULT_CONFIG_FILE: &str = "settings.yaml";

/// create configuration
/// priority list : command line > environment variables (todo) > config file
pub fn init_conf() -> Conf {
    const CARGO_PKG_VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");
    // parse args
    let clap_params = clap_args(CARGO_PKG_VERSION);
    // debug mode
    if clap_params.get_flag("verbose") {
        env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .format_target(false)
            .format_timestamp(None)
            .init();
    } else {
        env_logger::init();
    }
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
    let mut library_path = clap_params
        .get_one::<String>("library_path")
        .expect("required")
        .to_owned();
    if library_path == DEFAULT_LIBRARY_PATH {
        if let Ok(settings) = Config::builder()
            .add_source(config::File::with_name(&config_file_path))
            .add_source(config::Environment::with_prefix("ELORAN"))
            .build()
        {
            library_path = settings.get_string("library_path").unwrap();
        };
    }
    // delete last char if it's `/`
    let library_path = library_path.trim_end_matches('/').to_string();

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
                .default_value(DEFAULT_LIBRARY_PATH),
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
