use std::convert::Infallible;
use std::env;

use http::Uri;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::client::legacy::Client;
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::net::TcpListener;

// todo refactor!
async fn hello(rq: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let method = rq.method().clone();
    let version = rq.version();
    let headers = rq.headers().clone();
    let bytes = rq.collect().await.expect("error").to_bytes();
    let mut rq1 = Request::builder()
        .method(method.clone())
        .uri("http://localhost:3001".parse::<Uri>().unwrap())
        .version(version)
        .body(Full::new(bytes.clone()))
        .unwrap();
    rq1.headers_mut().extend(headers.clone());

    let mut rq2 = Request::builder()
        .method(method.clone())
        .uri("http://localhost:3002".parse::<Uri>().unwrap())
        .version(version)
        .body(Full::new(bytes.clone()))
        .unwrap();
    rq2.headers_mut().extend(headers.clone());

    let (_first, _second) = tokio::join!(
        async {
            let cl = Client::builder(TokioExecutor::new()).build_http();
            cl.request(rq1).await
        },
        async {
            let cl = Client::builder(TokioExecutor::new()).build_http();
            cl.request(rq2).await
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

    // We create a TcpListener and bind it to 127.0.0.1:3000
    let listener = TcpListener::bind(addr).await?;

    // We start a loop to continuously accept incoming connections
    loop {
        let (stream, _) = listener.accept().await?;

        // Use an adapter to access something implementing `tokio::io` traits as if they implement
        // `hyper::rt` IO traits.
        let io = TokioIo::new(stream);

        // Spawn a tokio task to serve multiple connections concurrently
        tokio::task::spawn(async move {
            // Finally, we bind the incoming connection to our `hello` service
            if let Err(err) = http1::Builder::new()
                // `service_fn` converts our function in a `Service`
                .serve_connection(io, service_fn(hello))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}
