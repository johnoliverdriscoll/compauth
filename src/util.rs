use futures_util::TryStreamExt;
use hyper::{
    Body, Client, Response, Request, StatusCode,
    client::HttpConnector,
};
use serde::{Serialize, Deserialize};

pub async fn to_bytes(body: Body) -> Option<Vec<u8>> {
    let mut stream = body.map_ok(|chunk| -> Vec<u8> {
        chunk.slice(0..chunk.len()).to_vec()
    });
    match stream.try_next().await {
        Ok(res) => res,
        Err(_) => None,
    }
}

pub fn from_bytes<'a, T: Deserialize<'a>>(
    bytes: Option<&'a Vec<u8>>
) -> Option<T> {
    match bytes {
        Some(bytes) => {
            match velocypack::from_bytes(bytes) {
                Ok(res) => Some(res),
                Err(_) => None,
            }
        }
        None => None,
    }
}

pub fn from_json<'a, T: Deserialize<'a>>(
    bytes: Option<&'a Vec<u8>>
) -> Option<T> {
    match bytes {
        Some(bytes) => match std::str::from_utf8(&bytes) {
            Ok(json) => match serde_json::from_str(json) {
                Ok(res) => Some(res),
                Err(x) => {
                    println!("{}", x);
                    None
                },
            },
            Err(_) => None,
        }
        None => None,
    }
}

pub struct AddrBaseClient {
    client: Client<HttpConnector, Body>,
    base: String,
}

impl AddrBaseClient {

    pub fn new(scheme: &str, addr: &str) -> AddrBaseClient {
        let mut base = String::new();
        base.push_str(scheme);
        base.push_str(addr);
        AddrBaseClient {
            client: Client::new(),
            base: base,
        }
    }

    pub async fn get(
        &mut self,
        path: &str
    ) -> Result<Response<Body>, &'static str> {
        let mut uri = self.base.clone();
        uri.push_str(path);
        let req = Request::get(uri).body(Body::empty()).unwrap();
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
        let req = Request::post(uri).body(Body::from(data)).unwrap();
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
        let req = Request::put(uri).body(Body::from(data)).unwrap();
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
