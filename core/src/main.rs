use std::env;

use http::Uri;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::client::legacy::Client;
use hyper_util::rt::{TokioExecutor, TokioIo};
use prelude::RqWrapper;
use tokio::net::TcpListener;

mod prelude;

async fn hello(rq: Request<Incoming>) -> Result<Response<Full<Bytes>>, String> {
    let wrap = RqWrapper::new(&rq);
    let bytes = rq.collect().await.expect("error").to_bytes();
    let (_first, _second) = tokio::join!(
        async {
            let cl = Client::builder(TokioExecutor::new()).build_http();
            cl.request(wrap.to_rq(&bytes, "http://localhost:3001".parse::<Uri>().unwrap()))
                .await
        },
        async {
            let cl = Client::builder(TokioExecutor::new()).build_http();
            cl.request(wrap.to_rq(&bytes, "http://localhost:3002".parse::<Uri>().unwrap()))
                .await
        }
    );
    Ok(Response::new(Full::new(Bytes::from("Hello, World!"))))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:3000".to_string());
    println!("Listening {}", addr);

    let listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, _) = listener.accept().await?;

        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(hello))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}
