use http::{HeaderMap, Method, Request, Uri, Version};
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};

pub struct RqWrapper {
    method: Method,
    version: Version,
    headers: HeaderMap,
}

impl RqWrapper {
    pub fn new(rq: &Request<Incoming>) -> Self {
        Self {
            method: rq.method().clone(),
            version: rq.version(),
            headers: rq.headers().clone(),
        }
    }

    pub fn to_rq(&self, body: &Bytes, uri: Uri) -> Request<Full<Bytes>> {
        let mut result = Request::builder()
            .method(self.method.clone())
            .version(self.version)
            .uri(uri)
            .body(Full::new(body.clone()))
            .unwrap();
        result.headers_mut().extend(self.headers.clone());
        result
    }
}
