use compauth::{
    authority::Authority,
    constant::AUTHORITY_ADDR,
    permission::Permission, 
    request::{UpdateRequest, ActionRequest},
    util::from_bytes,
};
use hyper::{
    Body, Error, Method, Request, Response, Server, StatusCode,
    body::to_bytes,
    service::{make_service_fn, service_fn},
};
use std::{convert::Infallible, sync::{Arc, atomic::AtomicPtr}};
use tokio::sync::Mutex;

async fn handle_key(
    m: Arc<Mutex<AtomicPtr<Authority>>>,
) -> Response<Body> {
    let auth = unsafe {
        (*m.lock().await).get_mut().as_ref().unwrap()
    };
    let resp = velocypack::to_bytes(auth.get_key()).unwrap();
    Response::new(resp.into())
}

async fn handle_add_perm(
    m: Arc<Mutex<AtomicPtr<Authority>>>,
    req: Request<Body>,
) -> Response<Body> {
    let bytes = to_bytes(req.into_body()).await;
    let perm: Permission = match from_bytes(&bytes) {
        Some(res) => res,
        None => {
            let mut bad_request = Response::default();
            *bad_request.status_mut() = StatusCode::BAD_REQUEST;
            return bad_request;
        },
    };
    let auth = unsafe {
        (*m.lock().await).get_mut().as_mut().unwrap()
    };
    let result = auth.add_permission(perm).await;
    let resp = velocypack::to_bytes(&result).unwrap();
    Response::new(resp.into())
}

async fn handle_update_perm(
    m: Arc<Mutex<AtomicPtr<Authority>>>,
    req: Request<Body>,
) -> Response<Body> {
    let bytes = to_bytes(req.into_body()).await;
    let req: UpdateRequest = match from_bytes(&bytes) {
        Some(res) => res,
        None => {
            let mut bad_request = Response::default();
            *bad_request.status_mut() = StatusCode::BAD_REQUEST;
            return bad_request;
        },
    };
    let auth = unsafe {
        (*m.lock().await).get_mut().as_mut().unwrap()
    };
    match auth.update_permission(req).await {
        Ok(result) => {
            let resp = velocypack::to_bytes(&result).unwrap();
            Response::new(resp.into())
        },
        _ => {
            let mut unauthorized = Response::default();
            *unauthorized.status_mut() = StatusCode::UNAUTHORIZED;
            unauthorized
        },
    }
}

async fn handle_action(
    m: Arc<Mutex<AtomicPtr<Authority>>>,
    req: Request<Body>,
) -> Response<Body> {
    let bytes = to_bytes(req.into_body()).await;
    let req: ActionRequest = match from_bytes(&bytes) {
        Some(res) => res,
        None => {
            let mut bad_request = Response::default();
            *bad_request.status_mut() = StatusCode::BAD_REQUEST;
            return bad_request;
        },
    };
    let auth = unsafe {
        (*m.lock().await).get_mut().as_mut().unwrap()
    };
    match auth.action(req).await {
        Ok(_) => Response::default(),
        _ => {
            let mut unauthorized = Response::default();
            *unauthorized.status_mut() = StatusCode::UNAUTHORIZED;
            unauthorized
        },
    }
}

async fn handle_update(
    m: Arc<Mutex<AtomicPtr<Authority>>>,
) -> Response<Body> {
    let auth = unsafe {
        (*m.lock().await).get_mut().as_mut().unwrap()
    };
    auth.update().await;
    Response::default()
}

async fn handle_sync(
    m: Arc<Mutex<AtomicPtr<Authority>>>,
) -> Response<Body> {
    let auth = unsafe {
        (*m.lock().await).get_mut().as_mut().unwrap()
    };
    auth.sync().await;
    Response::default()
}

async fn handle(
    m: Arc<Mutex<AtomicPtr<Authority>>>,
    req: Request<Body>,
) -> Result<Response<Body>, Error> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/key") => Ok(handle_key(m).await),
        (&Method::POST, "/permission") => Ok(handle_add_perm(m, req).await),
        (&Method::PUT, "/permission") => Ok(handle_update_perm(m, req).await),
        (&Method::POST, "/action") => Ok(handle_action(m, req).await),
        (&Method::GET, "/update") => Ok(handle_update(m).await),
        (&Method::GET, "/sync") => Ok(handle_sync(m).await),
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

#[tokio::main]
async fn main() {
    let mut authority = Authority::new();
    let m = Arc::new(Mutex::new(AtomicPtr::new(&mut authority)));
    let make_service = make_service_fn(move |_| {
        let m = Arc::clone(&m);
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle(Arc::clone(&m), req)
            }))
        }
    });
    let addr = AUTHORITY_ADDR.parse().unwrap();
    let server = Server::bind(&addr).serve(make_service);
    server.await.unwrap();
}
