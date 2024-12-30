use structopt::StructOpt;
use serde::Deserialize;
use std::fs;

#[derive(Debug, StructOpt)]
#[structopt(name = "dev-server", about = "A simple development server.")]
struct Opt {
    /// Address to listen on
    #[structopt(short, long)]
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

    let config: Config = if let Some(listen) = opt.listen {
        Config { listen_address: listen }
    } else {
        let config_content = fs::read_to_string(&opt.config)
            .expect("Failed to read configuration file");
        serde_yaml::from_str(&config_content)
            .expect("Failed to parse configuration file")
    };

    println!("Listening on: {}", config.listen_address);
}
