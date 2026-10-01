use crate::Error;
use axum::Router;
use hyper::server::conn::http1;
use hyper_util::{
    rt::{TokioIo, TokioTimer},
    service::TowerToHyperService,
};
use std::{
    future::Future,
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
        let connection = builder
            .serve_connection(
                TokioIo::new(stream),
                TowerToHyperService::new(router.clone()),
            )
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
