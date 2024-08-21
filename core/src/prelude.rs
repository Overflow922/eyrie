use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use http::Uri;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use tokio::time::timeout;
use anyhow::{anyhow, Result};

/**
 * Трейт для работы с конфигами роутов для приложения.
 * Роут - это пара входящий адрес -- список адресатов,
 * на который надо переслать запрос.
 *
 * Пример:
 * Входящий запрос /foo/bar
 * Перенаправляем на:
 * https://some-host:9090/foo/bar,
 * https://another-host:9898/foo/bar
 */
pub trait Config {
    /**
     * По урлу понимает на какие адреса надо переслать запросы
     */
    fn get_dests(&self, source_addr: &String) -> Option<&Vec<String>>;

    /**
     * Добавляет пару урл - новые адресаты
     */
    fn add_dests(&mut self, source_addr: String, destinations: Vec<String>);
}

#[derive(Default, Clone)]
pub struct HashMapConfig {
    dict: HashMap<String, Vec<String>>,
}

impl Config for HashMapConfig {
    fn get_dests(&self, source_addr: &String) -> Option<&Vec<String>> {
        self.dict.get(source_addr)
    }

    fn add_dests(&mut self, source_addr: String, destinations: Vec<String>) {
        self.dict.insert(source_addr, destinations);
    }
}

#[derive(Clone, Copy)]
pub struct ConfigLoader {}

impl ConfigLoader {
    pub fn load_from_file(filename: &str) -> Result<impl Config> {
        println!("Start loading config {filename}");
        let mut config = HashMapConfig::default();
        if let Ok(lines) = Self::read_lines(filename) {
            for line in lines.flatten() {
                let sides: Vec<_> = line.split("->").collect();
                if sides.len() == 2 {
                    let url = sides[0].trim();
                    let destinations = sides[1]
                        .trim()
                        .split(',')
                        .into_iter()
                        .map(|el| el.trim().to_string())
                        .collect::<Vec<String>>();
                    println!("Adding pair {:?} -> {:?}", url, destinations);
                    config.add_dests(url.to_string(), destinations)
                } else {
                    eprintln!("wrong line. {}. Skipped", line);
                }
            }
            println!("Done.");
            Ok(config)
        } else {
            Err(anyhow!("failed read config file"))
        }
    }

    fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
    where
        P: AsRef<Path>,
    {
        let file = File::open(filename)?;
        Ok(io::BufReader::new(file).lines())
    }
}

pub trait Router<C>
where
    C: Config,
{
    async fn route1(&self, rq: Request<Incoming>) -> Result<Response<Full<Bytes>>>;
    async fn route(
        &self,
        header: http::request::Parts,
        body: Incoming,
        destinations: Vec<String>,
    ) -> Result<Response<Full<Bytes>>>;
}

#[derive(Clone)]
pub struct ProxyRouter<C>(Arc<Mutex<C>>)
where
    C: Config;

impl<C> ProxyRouter<C>
where
    C: Config,
{
    pub fn new(config: C) -> Self {
        ProxyRouter(Arc::new(Mutex::new(config)))
    }
}

impl<C> ProxyRouter<C>
where
    C: Config,
{
    fn create_err_rs(err: String) -> Result<Response<Full<Bytes>>> {
        Result::Err(anyhow!(err))
    }

    async fn handle_rses(
        primary_rs: Result<Response<Incoming>>,
        secondary_rs: Result<Response<Incoming>>,
    ) -> Result<Response<Full<Bytes>>> {
        // get first request - supposed to be old service
        let (head, body) = match primary_rs {
            Ok(val) => val.into_parts(),
            Err(err) => return Self::create_err_rs(err.to_string()),
        };
        let bd = body.collect().await?.to_bytes();

        match secondary_rs {
            Ok(val) => {
                let el = val.into_parts().1.collect().await?.to_bytes();
                if el != bd {
                    eprintln!("rses differs");
                };
            },
            Err(err) => eprintln!("error {}", err.to_string()),
        };

        Ok(Response::from_parts(head, Full::new(bd)))
    }

    fn to_uri(uri: &str) -> Uri {
        uri.parse::<Uri>().unwrap()
    }

    fn resolve_path(h: &http::request::Parts) -> &str {
        h.uri.path()
    }
}

impl<C> Router<C> for ProxyRouter<C>
where
    C: Config,
{
    async fn route1(&self, rq: Request<Incoming>) -> Result<Response<Full<Bytes>>> {
        let (h, b) = rq.into_parts();
        let lock = self.0.lock().unwrap();
        let dests = lock.get_dests(&Self::resolve_path(&h).to_string());
        if let Some(v) = dests {
            self.route(h.clone(), b, v.clone()).await
        } else {
            Err(anyhow!("can't find route"))
        }
    }

    async fn route(
        &self,
        mut header: http::request::Parts,
        body: Incoming,
        dests: Vec<String>, // assume the size is 2
    ) -> Result<Response<Full<Bytes>>> {
        let bytes = body.collect().await.expect("msg").to_bytes(); // can we avoid copying bytes?

        let (original_rs, secondary_rs) = tokio::join!(
            {
                println!("doing rq {}", dests[0]);
                header.uri = Self::to_uri(dests[0].clone().as_str());
                let value = header.clone();
                let b = bytes.clone();
                async move { fun_name(value, b).await }
            },
            {
                println!("doing rq {}", dests[1]);
                header.uri = Self::to_uri(dests[1].clone().as_str());
                let value = header.clone();
                let b = bytes.clone();
                async move { fun_name(value, b).await }
            }
        );

        Self::handle_rses(original_rs, secondary_rs).await
    }
}

async fn fun_name(value: http::request::Parts, b: Bytes) -> Result<Response<Incoming>> {
    let cl = Client::builder(TokioExecutor::new()).build_http();
    let res = timeout(
        Duration::from_secs(1),
        cl.request(Request::from_parts(value.clone(), Full::new(b.clone()))),
    )
    .await;
    match res {
        Ok(r) => match r {
            Ok(rr) => Ok(rr),
            Err(err) => Err(anyhow!(err)),
        },
        Err(err) => Err(anyhow!(err)),
    }
}
