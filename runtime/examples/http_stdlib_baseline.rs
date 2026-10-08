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

async fn health(_: http::Request, _: Arc<State>, _: ()) -> Result<http::Response, rt::Error> {
    Ok(http::text(http::Status::OK, "ok"))
}
async fn small(_: http::Request, _: Arc<State>, _: ()) -> Result<http::Response, rt::Error> {
    let user = User {
        id: 1,
        name: "alice".into(),
        age: 18,
    };
    http::json(http::Status::OK, &user)
}
async fn echo(request: http::Request, _: Arc<State>, _: ()) -> Result<http::Response, rt::Error> {
    let user: CreateUser = rt::decode(request.body())?;
    http::json(http::Status::OK, &user)
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
        let app = http::route(
            app,
            http::Method::GET,
            "/small",
            http::public_policy(),
            small,
        )?;
        let app = http::route(
            app,
            http::Method::POST,
            "/echo",
            http::public_policy(),
            echo,
        )?;
        let port =
            rt::parse_i64(&std::env::var("NAGI_SAMPLE_PORT").unwrap_or_else(|_| "8086".into()))?;
        http::serve(app, port, http::default_options()).await
    })
    .unwrap();
}
