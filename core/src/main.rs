use std::env;
use std::time::Duration;

use http::Uri;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::client::legacy::{Client, Error};
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::net::TcpListener;
use tokio::time::error::Elapsed;
use tokio::time::timeout;

mod prelude;

async fn hello(rq: Request<Incoming>) -> Result<Response<Full<Bytes>>, String> {
    let (h, b) = rq.into_parts();
    let bytes = Box::new(b.collect().await.expect("msg").to_bytes());
    let (_first, _second) = tokio::join!(
        async {
            let cl = Client::builder(TokioExecutor::new()).build_http();
            let mut h1 = h.clone();
            h1.uri = to_uri("http://localhost:3001");
            timeout(
                Duration::from_secs(1),
                cl.request(Request::from_parts(h1, Full::new(Box::clone(&bytes)))),
            )
            .await
        },
        async {
            let cl = Client::builder(TokioExecutor::new()).build_http();
            let mut h1 = h.clone();
            h1.uri = to_uri("http://localhost:3002");
            timeout(
                Duration::from_secs(1),
                cl.request(Request::from_parts(h1, Full::new(Box::clone(&bytes)))),
            )
            .await
        }
    );

    check_result(_first, _second).await
}

async fn check_result(
    first: Result<Result<Response<Incoming>, Error>, Elapsed>,
    second: Result<Result<Response<Incoming>, Error>, Elapsed>,
) -> Result<Response<Full<Bytes>>, String> {
    match (first, second) {
        (Ok(Ok(rs)), Ok(Ok(rs2))) => choose_rs(rs, rs2).await,
        (Ok(Ok(rs)), Ok(Err(err))) | (Ok(Err(err)), Ok(Ok(rs))) => {
            create_rs(rs, err.to_string()).await
        }
        (Ok(Err(err)), Ok(Err(_))) => create_err_rs(err.to_string()),
        (Err(err), Err(_)) => create_err_rs(err.to_string()),
        (Ok(rs), Err(err)) | (Err(err), Ok(rs)) => process_part_success(rs, err.to_string()).await,
    }
}

async fn process_part_success(
    rs: Result<Response<Incoming>, Error>,
    err: String,
) -> Result<Response<Full<Bytes>>, String> {
    match rs {
        Ok(r) => create_rs(r, err.to_string()).await,
        Err(_) => create_err_rs(err.to_string()),
    }
}

async fn create_rs(rs: Response<Incoming>, err: String) -> Result<Response<Full<Bytes>>, String> {
    eprint!("error {}", err);
    let (parts, body) = rs.into_parts();
    let body = body.collect().await.expect("msg").to_bytes();
    Ok(Response::from_parts(parts, Full::new(body)))
}

fn create_err_rs(err: String) -> Result<Response<Full<Bytes>>, String> {
    Err(err.to_string())
}

async fn choose_rs(
    left: Response<Incoming>,
    right: Response<Incoming>,
) -> Result<Response<Full<Bytes>>, String> {
    let (parts_l, body_l) = left.into_parts();
    let (_, body_r) = right.into_parts();
    let left = body_l.collect().await.expect("msg").to_bytes();
    let right = body_r.collect().await.expect("msg").to_bytes();
    if left == right {
        Ok(Response::from_parts(parts_l, Full::new(left.clone())))
    } else {
        eprint!("rq2 differs from rq1!");
        Ok(Response::from_parts(parts_l, Full::new(left.clone())))
    }
}

fn to_uri(uri: &str) -> Uri {
    uri.parse::<Uri>().unwrap()
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
