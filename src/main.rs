use structopt::StructOpt;
use serde::Deserialize;
use std::fs;
use log::{info, LevelFilter};
use simplelog::{Config as LogConfig, TermLogger, TerminalMode};

#[derive(Debug, StructOpt)]
#[structopt(name = "dev-server", about = "A simple development server.")]
struct Opt {
    /// Address to listen on
    #[structopt(short = "l", long = "listen-address")]
    listen: Option<String>,

    /// Path to the configuration file
    #[structopt(short, long, default_value = "server.yml")]
    config: String,
}

#[derive(Debug, Deserialize)]
struct Config {
    listen_address: String,
}

fn main() {
    let opt: Opt  = Opt::from_args();

    let mut config: Config = {
        let config_content: String = if fs::metadata(&opt.config).is_ok() {
            fs::read_to_string(&opt.config)
            .expect("Failed to read configuration file")
        } else {
            String::new()
        };
        serde_yaml::from_str(&config_content)
            .expect("Failed to parse configuration file")
    };

    if let Some(listen) = opt.listen {
        config.listen_address = listen;
    }

    TermLogger::init(LevelFilter::Info, LogConfig::default(), TerminalMode::Mixed, simplelog::ColorChoice::Auto)
        .expect("Failed to initialize logger");

    info!("Listening on: {}", config.listen_address);
}
