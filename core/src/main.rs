use std::{
    env,
    sync::{Arc, Mutex},
};

use crate::prelude::Router;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use prelude::{ConfigLoader, ProxyRouter};
use tokio::net::TcpListener;

mod prelude;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:3000".to_string());
    let config_path = env::args()
        .nth(2)
        .unwrap_or_else(|| "config.txt".to_string());

    let router = Arc::new(Mutex::new(ProxyRouter::new(
        ConfigLoader::load_from_file(&config_path).expect("failed to load config"),
    )));

    println!("Listening {}", addr);
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, _) = listener.accept().await?;

        let io = TokioIo::new(stream);

        let r = router.clone();

        let kk = {
            let lock = r.lock().unwrap();
            let r = lock;
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(|rq| r.route1(rq)))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        };

        tokio::task::spawn(async move { kk });
    }
}
