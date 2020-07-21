use clacc::gmp::BigInt;
use compauth::{
    constant::WORKER_ADDR,
    permission::{Nonce, Permission},
    request::UpdateResponse,
    util::{to_bytes, from_bytes},
    worker::Worker,
};
use hyper::{
    Body, Error, Method, Request, Response, Server, StatusCode,
    service::{make_service_fn, service_fn},
};
use tokio::sync::Mutex;
use std::{convert::Infallible, sync::{Arc, atomic::AtomicPtr}};

async fn handle_key(
    m: Arc<Mutex<AtomicPtr<Worker>>>,
    req: Request<Body>,
) -> Response<Body> {
    let bytes = to_bytes(req.into_body()).await;
    let key: BigInt = match from_bytes(bytes.as_ref()) {
        Some(res) => res,
        None => {
            let mut bad_request = Response::default();
            *bad_request.status_mut() = StatusCode::BAD_REQUEST;
            return bad_request;
        },
    };
    let worker = unsafe {
        (*m.lock().await).get_mut().as_mut().unwrap()
    };
    match worker.set_key(key).await {
        Ok(_) => Response::default(),
        _ => {
            let mut forbidden = Response::default();
            *forbidden.status_mut() = StatusCode::FORBIDDEN;
            forbidden
        },
    }
}

async fn handle_add_perm(
    m: Arc<Mutex<AtomicPtr<Worker>>>,
    req: Request<Body>,
) -> Response<Body> {
    let bytes = to_bytes(req.into_body()).await;
    let perm: Permission = match from_bytes(bytes.as_ref()) {
        Some(res) => res,
        None => {
            let mut bad_request = Response::default();
            *bad_request.status_mut() = StatusCode::BAD_REQUEST;
            return bad_request;
        },
    };
    let worker = unsafe {
        (*m.lock().await).get_mut().as_mut().unwrap()
    };
    match worker.add_permission(perm).await {
        Ok(_) => Response::default(),
        _ => {
            let mut forbidden = Response::default();
            *forbidden.status_mut() = StatusCode::FORBIDDEN;
            forbidden
        },
    }
}

async fn handle_update_perm(
    m: Arc<Mutex<AtomicPtr<Worker>>>,
    req: Request<Body>,
) -> Response<Body> {
    let bytes = to_bytes(req.into_body()).await;
    let res: UpdateResponse = match from_bytes(bytes.as_ref()) {
        Some(res) => res,
        None => {
            let mut bad_request = Response::default();
            *bad_request.status_mut() = StatusCode::BAD_REQUEST;
            return bad_request;
        },
    };
    let worker = unsafe {
        (*m.lock().await).get_mut().as_mut().unwrap()
    };
    match worker.update_permission(res).await {
        Ok(_) => Response::default(),
        _ => {
            let mut forbidden = Response::default();
            *forbidden.status_mut() = StatusCode::FORBIDDEN;
            forbidden
        },
    }
}

async fn handle_witness(
    m: Arc<Mutex<AtomicPtr<Worker>>>,
    nonce: Nonce,
) -> Response<Body> {
    let worker = unsafe {
        (*m.lock().await).get_mut().as_mut().unwrap()
    };
    match worker.witness(nonce).await {
        Ok(res) => match res {
            Some(witness) => {
                let resp = velocypack::to_bytes(&witness).unwrap();
                Response::new(resp.into())
            },
            None => {
                let mut unauthorized = Response::default();
                *unauthorized.status_mut() = StatusCode::UNAUTHORIZED;
                unauthorized
            },
        },
        _ => {
            let mut forbidden = Response::default();
            *forbidden.status_mut() = StatusCode::FORBIDDEN;
            forbidden
        },
    }
}

async fn handle_update(
    m: Arc<Mutex<AtomicPtr<Worker>>>,
) -> Response<Body> {
    let worker = unsafe {
        (*m.lock().await).get_mut().as_mut().unwrap()
    };
    match worker.update().await {
        Ok(_) => Response::default(),
        _ => {
            let mut forbidden = Response::default();
            *forbidden.status_mut() = StatusCode::FORBIDDEN;
            forbidden
        },
    }
}

async fn handle_sync(
    m: Arc<Mutex<AtomicPtr<Worker>>>,
) -> Response<Body> {
    let worker = unsafe {
        (*m.lock().await).get_mut().as_mut().unwrap()
    };
    worker.sync().await;
    Response::default()
}

async fn handle(
    m: Arc<Mutex<AtomicPtr<Worker>>>,
    req: Request<Body>,
) -> Result<Response<Body>, Error> {
    match (req.method(), req.uri().path()) {
        (&Method::POST, "/key") => Ok(handle_key(m, req).await),
        (&Method::POST, "/permission") => Ok(handle_add_perm(m, req).await),
        (&Method::PUT, "/permission") => Ok(handle_update_perm(m, req).await),
        (&Method::GET, "/update") => Ok(handle_update(m).await),
        (&Method::GET, "/sync") => Ok(handle_sync(m).await),
        _ => {
            let path_bytes = req.uri().path().as_bytes();
            if path_bytes.len() > 0 && path_bytes[0] == b'/' {
                let parts: Vec<&str> = req.uri().path().split("/").collect();
                if parts.len() == 3 {
                    match parts[1] {
                        "witness" => match parts[2].parse::<u64>() {
                            Ok(nonce) => {
                                return Ok(handle_witness(
                                    m,
                                    nonce.into()
                                ).await);
                            },
                            _ => {},
                        },
                        _ => {},
                    }
                }
            }
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

#[tokio::main]
async fn main() {
    let mut worker = Worker::new();
    let m = Arc::new(Mutex::new(AtomicPtr::new(&mut worker)));
    let make_service = make_service_fn(move |_| {
        let m = Arc::clone(&m);
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle(Arc::clone(&m), req)
            }))
        }
    });
    let addr = WORKER_ADDR.parse().unwrap();
    let server = Server::bind(&addr).serve(make_service);
    server.await.unwrap();
}
