use structopt::StructOpt;
use serde::Deserialize;
use std::fs;
use log::{info, LevelFilter};
use simplelog::{Config as LogConfig, TermLogger, TerminalMode};
use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use hyper::body::to_bytes;

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

#[tokio::main]
async fn main() {
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

    let make_svc = make_service_fn(|_conn: &hyper::server::conn::AddrStream| {
        async {
            Ok::<_, hyper::Error>(service_fn(|mut req: Request<Body>| {
                async move {
                    let whole_body = to_bytes(req.body_mut()).await?;
                    let body_str = match String::from_utf8(whole_body.to_vec()) {
                        Ok(body) => body,
                        Err(e) => {
                            eprintln!("Request body is not valid UTF-8: {}", e);
                            return Ok(Response::new(Body::from("Invalid UTF-8 in request body")));
                        }
                    };
                    {
                        // print the request details
                        let method = req.method(); // Borrowing reference
                        let uri = req.uri();       // Borrowing reference
                        let version = req.version(); // Borrowing reference
                        let headers = req.headers(); // Borrowing reference

                        info!("--- New Request ---\n\nMethod: {}\nURI: {}\nVersion: {:?}\nHeaders: {:?}\nBody: {}\n", method, uri, version, headers, body_str);
                    }
                    Ok::<_, hyper::Error>(Response::new(Body::from("Server Response")))
                }
            }))
        }
    });

    let addr: std::net::SocketAddr = config.listen_address.parse().expect("Invalid listen address");
    let server = Server::bind(&addr).serve(make_svc);

    info!("Server running on {}", addr);

    if let Err(e) = server.await {
        eprintln!("Server error: {}", e);
    }
}
