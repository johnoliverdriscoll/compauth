use compauth::{
    synchronizer::Synchronizer,
    constant::SYNCHRONIZER_ADDR,
    permission::{Action, Permission},
    util::from_json,
};
use hyper::{
    Body, Error, Method, Request, Response, Server, StatusCode,
    body::to_bytes,
    service::{make_service_fn, service_fn},
};
use serde::Deserialize;
use std::{convert::Infallible, sync::{Arc, atomic::AtomicPtr}};
use tokio::sync::Mutex;

#[derive(Deserialize)]
struct UpdateRequest {
    perm: Permission,
    actions: Vec<Action>,
}

#[derive(Deserialize)]
struct ActionRequest {
    perm: Permission,
    action: Action,
}

async fn handle_add_perm(
    m: Arc<Mutex<AtomicPtr<Synchronizer>>>,
    req: Request<Body>,
) -> Response<Body> {
    let bytes = to_bytes(req.into_body()).await;
    let actions: Vec<Action> = match from_json(&bytes) {
        Some(res) => res,
        None => {
            let mut bad_request = Response::default();
            *bad_request.status_mut() = StatusCode::BAD_REQUEST;
            return bad_request;
        },
    };
    let sync = unsafe {
        (*m.lock().await).get_mut().as_mut().unwrap()
    };
    match sync.add_permission(actions).await {
        Ok(res) => Response::new(serde_json::to_string(&res).unwrap().into()),
        _ => {
            let mut unauthorized = Response::default();
            *unauthorized.status_mut() = StatusCode::UNAUTHORIZED;
            unauthorized
        },
    }
}

async fn handle_update_perm(
    m: Arc<Mutex<AtomicPtr<Synchronizer>>>,
    req: Request<Body>,
) -> Response<Body> {
    let bytes = to_bytes(req.into_body()).await;
    let req: UpdateRequest = match from_json(&bytes) {
        Some(res) => res,
        None => {
            let mut bad_request = Response::default();
            *bad_request.status_mut() = StatusCode::BAD_REQUEST;
            return bad_request;
        },
    };
    let sync = unsafe {
        (*m.lock().await).get_mut().as_mut().unwrap()
    };
    match sync.update_permission(req.perm, req.actions).await {
        Ok(res) => Response::new(serde_json::to_string(&res).unwrap().into()),
        _ => {
            let mut unauthorized = Response::default();
            *unauthorized.status_mut() = StatusCode::UNAUTHORIZED;
            unauthorized
        },
    }
}

async fn handle_action(
    m: Arc<Mutex<AtomicPtr<Synchronizer>>>,
    req: Request<Body>,
) -> Response<Body> {
    let bytes = to_bytes(req.into_body()).await;
    let req: ActionRequest = match from_json(&bytes) {
        Some(res) => res,
        None => {
            let mut bad_request = Response::default();
            *bad_request.status_mut() = StatusCode::BAD_REQUEST;
            return bad_request;
        },
    };
    let sync = unsafe {
        (*m.lock().await).get_mut().as_mut().unwrap()
    };
    match sync.action(req.perm, req.action).await {
        Ok(_) => Response::default(),
        _ => {
            let mut unauthorized = Response::default();
            *unauthorized.status_mut() = StatusCode::UNAUTHORIZED;
            unauthorized
        },
    }
}    

async fn handle(
    m: Arc<Mutex<AtomicPtr<Synchronizer>>>,
    req: Request<Body>,
) -> Result<Response<Body>, Error> {
    match (req.method(), req.uri().path()) {
        (&Method::POST, "/permission") => Ok(handle_add_perm(m, req).await),
        (&Method::PUT, "/permission") => Ok(handle_update_perm(m, req).await),
        (&Method::POST, "/action") => Ok(handle_action(m, req).await),
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

#[tokio::main]
async fn main() {
    let mut sync = Synchronizer::new().await.unwrap();
    let m = Arc::new(Mutex::new(AtomicPtr::new(&mut sync)));
    let make_service = make_service_fn(move |_| {
        let m = Arc::clone(&m);
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle(Arc::clone(&m), req)
            }))
        }
    });
    let addr = SYNCHRONIZER_ADDR.parse().unwrap();
    let server = Server::bind(&addr).serve(make_service);
    let sync_future = sync.sync();
    server.await.unwrap();
    sync_future.await.unwrap().unwrap();
}
