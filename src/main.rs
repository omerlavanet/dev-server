use structopt::StructOpt;
use serde::Deserialize;
use std::fs;
use log::{info, LevelFilter, warn};
use simplelog::{Config as LogConfig, TermLogger, TerminalMode};
use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use hyper::body::to_bytes;
use hyper::client::Client;
use tokio::time::{timeout, Duration};
use futures::future::select_all;
use uuid::Uuid;

#[derive(Debug, StructOpt)]
#[structopt(name = "dev-server", about = "A simple development server.")]
struct Opt {
    /// Address to listen on
    #[structopt(short = "l", long = "listen-address")]
    listen: Option<String>,

    /// Path to the configuration file
    #[structopt(short, long, default_value = "server.yml")]
    config: String,

    /// Proxy destinations, where traffic will be routed to, if more than one is provided the traffic is mirrored
    #[structopt(short, long)]
    proxy: Vec<String>,

    /// Default response when no proxy destinations are set
    #[structopt(short="dr",long)]
    default_response: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Config {
    listen_address: Option<String>,
    proxy_destinations: Option<Vec<String>>,
    default_response: Option<String>,
}

#[tokio::main]
async fn main() {
    let opt: Opt  = Opt::from_args();

    let config: Config = {
        let config_content: String = if fs::metadata(&opt.config).is_ok() {
            fs::read_to_string(&opt.config)
            .expect("Failed to read configuration file")
        } else {
            String::new()
        };
        let mut config: Config = serde_yaml::from_str(&config_content)
            .expect("Failed to parse configuration file");

        if let Some(listen) = opt.listen {
            config.listen_address = Some(listen);
        }

        if !opt.proxy.is_empty() {
            config.proxy_destinations = Some(opt.proxy);
        }

        if let Some(mut proxy_destinations_vec) = config.proxy_destinations {
            proxy_destinations_vec.retain(|proxy| {
                !proxy.is_empty() && proxy != ""
            });
            if proxy_destinations_vec.len() == 0 {
                config.proxy_destinations = None;
            } else {
                config.proxy_destinations = Some(proxy_destinations_vec);
            }
        }

        if let Some(default_resp) = opt.default_response {
            config.default_response = Some(default_resp);
        } else {
            config.default_response = Some("Default Server Response".to_string());
        }

        config
    };

    TermLogger::init(LevelFilter::Info, LogConfig::default(), TerminalMode::Mixed, simplelog::ColorChoice::Auto)
        .expect("Failed to initialize logger");

    if config.proxy_destinations.is_none() {
        let resp_log :&String= config.default_response.as_ref().expect("must be non empty is is Some");
        info!(
            "Starting server on {} in default mode with default response: {}.",
            config.listen_address.as_ref().unwrap(),resp_log
        );
    } else {
        info!(
            "Starting server on {} with proxies to: {:?}",
            config.listen_address.as_ref().unwrap(),
            config.proxy_destinations
        );
    }

    let default_response = config.default_response.unwrap();

    let make_svc = make_service_fn(move |_conn: &hyper::server::conn::AddrStream| {
        let client = Client::new();
        let proxy_destinations  = config.proxy_destinations.clone();
        let default_response = default_response.clone();
        async move {
            Ok::<_, hyper::Error>(service_fn(move |mut req: Request<Body>| {
                let client = client.clone();
                let proxy_destinations = proxy_destinations.clone();
                let default_response = default_response.clone();
                async move {
                    let whole_body = to_bytes(req.body_mut()).await?;
                    let body_str = match String::from_utf8(whole_body.to_vec()) {
                        Ok(body) => body,
                        Err(e) => {
                            eprintln!("Request body is not valid UTF-8: {}", e);
                            return Ok(Response::new(Body::from("Invalid UTF-8 in request body")));
                        }
                    };
                    
                    // print the request details
                    let method = req.method(); // Borrowing reference
                    let uri = req.uri();       // Borrowing reference
                    let version = req.version(); // Borrowing reference
                    let headers = req.headers(); // Borrowing reference
                    let request_id = Uuid::new_v4();
                    info!("--- New Request [{}] ---\n\nMethod: {}\nURI: {}\nVersion: {:?}\nHeaders: {:?}\nBody: {}\n", request_id, method, uri, version, headers, body_str);
                    
                    if proxy_destinations.is_some() {
                        let timeout_duration = Duration::from_secs(30);
                        let mut futures = proxy_destinations.unwrap().into_iter().map(|proxy| {
                            let client = client.clone();
                            // Construct the absolute URI for the proxied request
                            let proxy_uri: hyper::Uri = proxy.parse().expect("Invalid proxy URI");
                            let absolute_uri = format!("{}://{}{}", proxy_uri.scheme_str().unwrap_or("http"), proxy_uri.authority().unwrap(), uri);
                            let mut new_req = Request::builder()
                                .method(req.method())
                                .uri(absolute_uri)
                                .version(req.version())
                                .body(Body::from(body_str.clone()))
                                .expect("Failed to build request");

                            *new_req.headers_mut() = req.headers().clone();

                            Box::pin(timeout(timeout_duration, client.request(new_req)))
                        }).collect::<Vec<_>>();

                        while !futures.is_empty() {
                            let (result, _, remaining_futures) = select_all(futures).await;
                            futures = remaining_futures;

                            match result {
                                Ok(Ok(mut response)) => {
                                    let body_bytes = to_bytes(response.body_mut()).await?;
                                    let status = response.status();
                                    let headers = response.headers();
                                    let body_str = String::from_utf8(body_bytes.to_vec()).expect("Response body is not valid UTF-8");

                                    info!("--- Got Response [{}] ---\n\nStatus: {}\nHeaders: {:?}\nBody: {}\n\n",request_id, status, headers, body_str);

                                    let new_response = Response::builder()
                                        .status(status)
                                        .body(Body::from(body_bytes))
                                        .expect("Failed to build response");

                                    return Ok(new_response);
                                },
                                Ok(Err(e)) => {
                                    warn!("Proxy request failed: {}", e);
                                },
                                Err(_) => {
                                    warn!("Proxy request timed out");
                                }
                            }
                        }

                        warn!("Returning default response as no valid response received from any proxy");
                    }
                    
                    Ok::<_, hyper::Error>(Response::new(Body::from(default_response)))
                }
            }))
        }
    });

    let addr: std::net::SocketAddr = config.listen_address.as_ref().unwrap().parse().expect("Invalid listen address");
    let server = Server::bind(&addr).serve(make_svc);

    info!("Server running on {}", addr);

    if let Err(e) = server.await {
        eprintln!("Server error: {}", e);
    }
}
