//! Benchmark-only custom Axum host reference, outside the standard HTTP dispatcher.
//!
//! This uses Axum's listener directly. It does not reproduce the retired
//! `rt::serve` wrapper's bind behavior, hidden endpoints, body bounds, or
//! lifecycle policy, and its old measurements are not SF01-equivalent data.
use nagi_runtime as rt;
use rt::axum::{
    body::Bytes,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
#[derive(Serialize)]
struct User {
    id: i64,
    name: String,
    age: i32,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateUser {
    name: String,
    age: i32,
}
fn main() {
    rt::block_on(async {
        let router = Router::new()
            .route(
                "/small",
                get(|| async {
                    rt::response(Ok(User {
                        id: 1,
                        name: "alice".into(),
                        age: 18,
                    }))
                }),
            )
            .route(
                "/echo",
                post(|body: Bytes| async move { rt::response(rt::decode::<CreateUser>(&body)) }),
            );
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 8082))
            .await
            .unwrap();
        axum::serve(listener, router).await.unwrap();
    });
}
