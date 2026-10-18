//! Direct Rust equivalent of benchmarks/http_stdlib.nagi.
use nagi_runtime::{self as rt, http_server as http};
use rt::serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize)]
#[serde(crate = "rt::serde")]
struct User {
    id: i64,
    name: String,
    age: i32,
}
#[derive(Deserialize, Serialize)]
#[serde(crate = "rt::serde", deny_unknown_fields)]
struct CreateUser {
    name: String,
    age: i32,
}
struct State {
    _ready: bool,
}

async fn health(request: http::Request, _: Arc<State>, _: ()) -> Result<http::Response, rt::Error> {
    http::text(http::response_context(&request), http::Status::OK, "ok")
}
async fn small(request: http::Request, _: Arc<State>, _: ()) -> Result<http::Response, rt::Error> {
    let user = User {
        id: 1,
        name: "alice".into(),
        age: 18,
    };
    http::json(http::response_context(&request), http::Status::OK, &user)
}
async fn echo(request: http::Request, _: Arc<State>, _: ()) -> Result<http::Response, rt::Error> {
    let user: CreateUser = rt::decode(request.body())?;
    http::json(http::response_context(&request), http::Status::OK, &user)
}
fn main() {
    rt::block_on(async {
        let app = http::app_default(State { _ready: true });
        let app = http::route(
            app,
            http::Method::GET,
            "/health",
            http::public_policy(),
            health,
        )?;
        let health_rate = http::rate_policy(1_000_000_000, 1)?;
        let app = http::rate_route(app, http::Method::GET, "/health", health_rate)?;
        let app = http::route(
            app,
            http::Method::GET,
            "/small",
            http::public_policy(),
            small,
        )?;
        let small_rate = http::rate_policy(1_000_000_000, 1)?;
        let app = http::rate_route(app, http::Method::GET, "/small", small_rate)?;
        let app = http::route(
            app,
            http::Method::POST,
            "/echo",
            http::public_policy(),
            echo,
        )?;
        let echo_rate = http::rate_policy(1_000_000_000, 1)?;
        let app = http::rate_route(app, http::Method::POST, "/echo", echo_rate)?;
        let port =
            rt::parse_i64(&std::env::var("NAGI_SAMPLE_PORT").unwrap_or_else(|_| "8086".into()))?;
        let wire_authority =
            std::env::var("NAGI_HTTP_AUTHORITY").unwrap_or_else(|_| "127.0.0.1:8086".into());
        let limits = http::authority(
            http::default_options(),
            "https://localhost",
            vec![wire_authority],
            1,
            256,
        )?;
        let response_limits = http::response_limits(
            1_048_576,
            8_192,
            32,
            8_192,
            32,
            65_536,
            64,
            83_886_080,
            1_024,
            285_212_672,
            8_192,
            32,
        )?;
        let limits = http::with_response_limits(limits, response_limits)?;
        let rate_limits = http::rate_limits(64, 64, 4096, 4_194_304, 65_536)?;
        let limits = http::with_rate_limits(limits, rate_limits)?;
        http::serve(app, port, limits).await
    })
    .unwrap();
}
