//! A custom Rust HTTP backend. Nagi owns the greeting policy, and Rust owns
//! routing, request limits, task placement, and listener lifetime.
use axum::{
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, FromRequest, Path, Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Extension, Router,
};
use std::{net::Ipv4Addr, sync::Arc, time::Duration};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const MAX_IN_FLIGHT: usize = 8;
const MAX_BODY_BYTES: usize = 64 * 1024;
const REQUEST_DEADLINE: Duration = Duration::from_secs(3);

#[derive(Clone)]
struct AppState {
    render: fn(i64) -> String,
    requests: Arc<Semaphore>,
}

type RequestSlot = Arc<OwnedSemaphorePermit>;

async fn request_bounds(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let permit = match state.requests.clone().try_acquire_owned() {
        Ok(permit) => Arc::new(permit),
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                [(header::RETRY_AFTER, "1")],
                "Server busy\n",
            )
                .into_response();
        }
    };
    request.extensions_mut().insert(permit.clone());

    let work = async {
        // Buffer even bodies on routes that would not otherwise extract them.
        // Axum's extractor reports oversized bodies as 413 and read errors as 400.
        let (parts, body) = request.into_parts();
        let mut body_request = Request::new(body);
        DefaultBodyLimit::max(MAX_BODY_BYTES).apply(&mut body_request);
        let bytes = match Bytes::from_request(body_request, &()).await {
            Ok(bytes) => bytes,
            Err(rejection) => return rejection.into_response(),
        };
        next.run(Request::from_parts(parts, Body::from(bytes)))
            .await
    };
    let response = match tokio::time::timeout(REQUEST_DEADLINE, work).await {
        Ok(response) => response,
        Err(_) => (StatusCode::REQUEST_TIMEOUT, "Request timed out\n").into_response(),
    };
    // A callback that outlives its response retains its own copy of the permit.
    drop(permit);
    response
}

async fn greeting(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Extension(slot): Extension<RequestSlot>,
) -> Response {
    // A synchronous callback belongs on a blocking worker. Keeping the permit
    // inside that task prevents timed-out callbacks from creating an unbounded
    // queue. Tokio cannot forcibly interrupt a callback that has already started.
    let render = state.render;
    match tokio::task::spawn_blocking(move || {
        let _slot = slot;
        format!("{} {id}\n", render(id))
    })
    .await
    {
        Ok(text) => text.into_response(),
        Err(error) => {
            eprintln!("Greeting callback failed: {error}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Callback failed\n").into_response()
        }
    }
}

pub async fn run_server(render: fn(i64) -> String, port: i64) -> Result<(), nagi_runtime::Error> {
    let port = u16::try_from(port)
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| nagi_runtime::Error::invalid("port must be an integer from 1 to 65535"))?;
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .await
        .map_err(|error| nagi_runtime::Error::internal(error.to_string()))?;
    let state = AppState {
        render,
        requests: Arc::new(Semaphore::new(MAX_IN_FLIGHT)),
    };
    let app = Router::new()
        .route("/health", get(|| async { "ok\n" }))
        .route("/hello/{id}", get(greeting))
        .fallback(|| async { (StatusCode::NOT_FOUND, "Route not found\n") })
        .layer(middleware::from_fn_with_state(
            state.clone(),
            request_bounds,
        ))
        .with_state(state);

    println!("Custom HTTP listening http://127.0.0.1:{port}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            if let Err(error) = tokio::signal::ctrl_c().await {
                eprintln!("Ctrl+C listener failed: {error}");
            }
        })
        .await
        .map_err(|error| nagi_runtime::Error::internal(error.to_string()))
}
