use axum::{
    extract::{DefaultBodyLimit, Json},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use std::{net::Ipv4Addr, time::Duration};

// This timer demonstrates Nagi awaiting a Rust async operation. It does not
// simulate a database or enforce a request deadline.
pub async fn pause() {
    tokio::time::sleep(Duration::from_millis(1)).await;
}

async fn quote(Json(input): Json<super::QuoteInput>) -> Response {
    // The adapter calls one known generated function by name, then awaits it.
    // It is not a generic async callback passed through Nagi extern.
    match super::calculate(input).await {
        Ok(output) => Json(output).into_response(),
        Err(super::QuoteError::InvalidQuantity) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(nagi_runtime::serde_json::json!({
                "code": "invalid_quantity",
                "message": "Quantity must be between 1 and 100",
            })),
        )
            .into_response(),
    }
}

pub async fn run_server(port: i64) -> Result<(), nagi_runtime::Error> {
    let port = u16::try_from(port)
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| nagi_runtime::Error::invalid("port must be between 1 and 65535"))?;
    let router = Router::new()
        .route("/health", get(|| async { "ok\n" }))
        .route("/quotes", post(quote))
        .layer(DefaultBodyLimit::max(4096));
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .await
        .map_err(|error| nagi_runtime::Error::internal(error.to_string()))?;
    println!("Axum service listening http://127.0.0.1:{port}");
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            if let Err(error) = tokio::signal::ctrl_c().await {
                eprintln!("Ctrl+C listener failed: {error}");
            }
        })
        .await
        .map_err(|error| nagi_runtime::Error::internal(error.to_string()))
}
