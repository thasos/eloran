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
pub fn init_conf(args: Vec<String>) -> Conf {
    const CARGO_PKG_VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");
    let mut logbuilder = env_logger::Builder::from_default_env();
    logbuilder.target(env_logger::Target::Stdout);
    // debug mode
    if env::var("RUST_LOG").is_err() {
        logbuilder.filter_level(log::LevelFilter::Info);
        // crates too verbose
        logbuilder.filter(Some("sqlx::"), log::LevelFilter::Off);
        logbuilder.filter(Some("hyper::"), log::LevelFilter::Off);
        // crates pdf and hypper are okay
    }
    match logbuilder.try_init() {
        Ok(_) => (),
        Err(_) => eprintln!("unable to initiate pretty logging"),
    };

    info!(
        "starting up version={}",
        CARGO_PKG_VERSION.unwrap_or("version not found")
    );

    match args.len() {
        2 => {
            // TODO usage message : `eloran [ip:port]
            let bind: String = args[1].to_string();
            let library_path = Conf::default().library_path;
            Conf { bind, library_path }
        }
        _ => Conf::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_conf() {
        insta::assert_yaml_snapshot!(init_conf(Vec::new()));
        insta::assert_yaml_snapshot!(init_conf(vec![
            "eloran".to_string(),
            "127.0.0.1:8080".to_string()
        ]));
    }
}
