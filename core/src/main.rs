use std::env;

use http::Uri;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::client::legacy::{Client, Error};
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::net::TcpListener;

mod prelude;

async fn hello(rq: Request<Incoming>) -> Result<Response<Full<Bytes>>, String> {
    let (h, b) = rq.into_parts();
    let bytes = b.collect().await.expect("msg").to_bytes(); // can we avoid copying bytes?
    let (_first, _second) = tokio::join!(
        async {
            let cl = Client::builder(TokioExecutor::new()).build_http();
            let mut h1 = h.clone();
            h1.uri = to_uri("http://localhost:3001");
            cl.request(Request::from_parts(h1, Full::new(bytes.clone())))
                .await
        },
        async {
            let cl = Client::builder(TokioExecutor::new()).build_http();
            let mut h1 = h.clone();
            h1.uri = to_uri("http://localhost:3002");
            cl.request(Request::from_parts(h1, Full::new(bytes.clone())))
                .await
        }
    );

    check_result(_first, _second).await
}

async fn check_result(
    first: Result<Response<Incoming>, Error>,
    second: Result<Response<Incoming>, Error>,
) -> Result<Response<Full<Bytes>>, String> {
    match first {
        Ok(rs) => match second {
            Ok(rs2) => choose_rs(rs, rs2).await,
            Err(e) => {
                eprint!("error {}", e);
                let (parts, body) = rs.into_parts();
                let body = body.collect().await.expect("msg").to_bytes();
                Ok(Response::from_parts(parts, Full::new(body)))
            }
        },
        Err(e) => Err(e.to_string()),
    }
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
