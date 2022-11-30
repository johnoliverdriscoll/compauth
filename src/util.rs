use hyper::{
    Body,
    Client as HyperClient,
    Method,
    Response,
    Request,
    StatusCode,
    body::Bytes,
    client::connect::HttpConnector,
};
use serde::{Serialize, Deserialize};

pub fn from_bytes<'a, T: Deserialize<'a>, E>(
    bytes: &'a Result<Bytes, E>,
) -> Option<T> {
    match bytes {
        Ok(bytes) => {
            match velocypack::from_bytes(&bytes[..]) {
                Ok(res) => Some(res),
                Err(_) => None,
            }
        }
        Err(_) => None,
    }
}

pub fn from_json<'a, T: Deserialize<'a>, E>(
    bytes: &'a Result<Bytes, E>,
) -> Option<T> {
    match bytes {
        Ok(bytes) => match std::str::from_utf8(&bytes[..]) {
            Ok(json) => match serde_json::from_str(json) {
                Ok(res) => Some(res),
                Err(_) => None
            },
            Err(_) => None,
        }
        Err(_) => None,
    }
}

pub struct Client {
    client: HyperClient<HttpConnector, Body>,
    base: String,
}

impl Client {

    pub fn new(host: &str) -> Client {
        let mut base = String::new();
        base.push_str("http://");
        base.push_str(host);
        Client {
            client: HyperClient::new(), 
            base: base,
        }
    }

    pub async fn get(
        &mut self,
        path: &str
    ) -> Result<Response<Body>, &'static str> {
        let mut uri = self.base.clone();
        uri.push_str(path);
        let req = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        match self.client.request(req).await {
            Ok(resp) => {
                match resp.status() {
                    StatusCode::OK => Ok(resp),
                    x => Err(x.canonical_reason().unwrap()),
                }
            },
            _ => Err("request error"),
        }
    }

    pub async fn post<T>(
        &mut self,
        path: &str,
        body: T,
    ) -> Result<Response<Body>, &'static str>
    where T: Serialize {
        let mut uri = self.base.clone();
        uri.push_str(path);
        let data = velocypack::to_bytes(&body).unwrap();
        let req = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .body(Body::from(data))
            .unwrap();
        match self.client.request(req).await {
            Ok(resp) => {
                match resp.status() {
                    StatusCode::OK => Ok(resp),
                    x => Err(x.canonical_reason().unwrap()),
                }
            },
            _ => Err("request error"),
        }
    }

    pub async fn put<T>(
        &mut self,
        path: &str,
        body: T,
    ) -> Result<Response<Body>, &'static str>
    where T: Serialize {
        let mut uri = self.base.clone();
        uri.push_str(path);
        let data = velocypack::to_bytes(&body).unwrap();
        let req = Request::builder()
            .method(Method::PUT)
            .uri(uri)
            .body(Body::from(data))
            .unwrap();
        match self.client.request(req).await {
            Ok(resp) => {
                match resp.status() {
                    StatusCode::OK => Ok(resp),
                    x => Err(x.canonical_reason().unwrap()),
                }
            },
            _ => Err("request error"),
        }
    }
}
