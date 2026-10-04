use crate::Error;
use axum::{
    body::Body,
    http::{header, HeaderValue, Method, StatusCode},
    response::Response,
    Router,
};
use futures_util::FutureExt;
use hyper::{
    server::conn::http1,
    service::{service_fn, Service},
};
use hyper_util::{
    rt::{TokioIo, TokioTimer},
    service::TowerToHyperService,
};
use std::{
    future::Future,
    panic::{catch_unwind, AssertUnwindSafe},
    time::{Duration, Instant},
};
use tokio::net::TcpListener;

const WAIT_SECONDS: &str = "NAGI_HTTP_REQUEST_WAIT_SECONDS";

fn parse_wait_timeout(value: Option<&str>) -> Result<Duration, Error> {
    let Some(value) = value else {
        return Ok(Duration::from_secs(10));
    };
    let seconds = value
        .parse::<u64>()
        .ok()
        .filter(|seconds| *seconds > 0)
        .ok_or_else(|| Error::invalid(format!("{WAIT_SECONDS} must be a positive integer")))?;
    let duration = Duration::from_secs(seconds);
    if Instant::now().checked_add(duration).is_none() {
        return Err(Error::invalid(format!("{WAIT_SECONDS} is too large")));
    }
    Ok(duration)
}

pub(crate) fn request_wait_timeout() -> Result<Duration, Error> {
    match std::env::var(WAIT_SECONDS) {
        Ok(value) => parse_wait_timeout(Some(&value)),
        Err(std::env::VarError::NotPresent) => parse_wait_timeout(None),
        Err(_) => Err(Error::invalid(format!(
            "{WAIT_SECONDS} must be a positive integer"
        ))),
    }
}

fn panic_response(head: bool) -> Response {
    let mut response = Response::new(if head {
        Body::empty()
    } else {
        Body::from("Internal Server Error")
    });
    *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
        .headers_mut()
        .insert(header::CONNECTION, HeaderValue::from_static("close"));
    if head {
        response
            .headers_mut()
            .insert(header::CONTENT_LENGTH, HeaderValue::from_static("21"));
    }
    response
}

/// A deadline for complete request headers, including the idle interval after
/// an HTTP response. Hyper owns parsing, buffering, streaming, and upgrades.
pub(crate) async fn serve(
    listener: TcpListener,
    router: Router,
    request_wait: Duration,
    shutdown_signal: impl Future<Output = ()>,
) {
    let (shutdown, _) = tokio::sync::watch::channel(());
    let mut builder = http1::Builder::new();
    builder
        .timer(TokioTimer::new())
        .header_read_timeout(request_wait);
    tokio::pin!(shutdown_signal);
    loop {
        let accepted = tokio::select! {
            biased;
            _ = &mut shutdown_signal => break,
            accepted = listener.accept() => accepted,
        };
        let (stream, _) = match accepted {
            Ok(accepted) => accepted,
            Err(error) => {
                // Keep the listener alive while temporary resource exhaustion
                // clears. Other connection tasks can still finish or expire.
                eprintln!("HTTP accept: {error}");
                tokio::select! {
                    _ = &mut shutdown_signal => break,
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {}
                }
                continue;
            }
        };
        let service = TowerToHyperService::new(router.clone());
        let service = service_fn(move |request: hyper::Request<hyper::body::Incoming>| {
            let head = request.method() == Method::HEAD;
            // The adapter returns an owned future. Keep this boundary on the
            // stack without an extra middleware box or per-request clone.
            let future = catch_unwind(AssertUnwindSafe(|| service.call(request)));
            async move {
                let result = match future {
                    Ok(future) => AssertUnwindSafe(future).catch_unwind().await,
                    Err(panic) => Err(panic),
                };
                match result {
                    Ok(response) => response,
                    // An unwind before response headers becomes a generic 500;
                    // application state is not rolled back by catching it.
                    Err(_) => Ok(panic_response(head)),
                }
            }
        });
        let connection = builder
            .serve_connection(TokioIo::new(stream), service)
            .with_upgrades();
        let mut stopped = shutdown.subscribe();
        tokio::spawn(async move {
            tokio::pin!(connection);
            let result = tokio::select! {
                result = &mut connection => result,
                _ = stopped.changed() => {
                    connection.as_mut().graceful_shutdown();
                    connection.await
                }
            };
            if let Err(error) = result {
                // An invalid request, expired wait, or disconnected peer is
                // routine input; avoid a log entry for every such connection.
                if error.is_user() {
                    eprintln!("HTTP connection: {error}");
                }
            }
        });
    }
    drop(listener);
    let _ = shutdown.send(());
    shutdown.closed().await;
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod panic_tests;
