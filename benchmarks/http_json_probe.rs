// Isolated measurement controls; the preceding generated source is unchanged
// except for its executable entry name. No HTTP server is started by this probe.
use nagi_runtime::{self as rt, http_server as native_http};
use rt::serde::{Deserialize, Serialize};
use std::{future::Future, hint::black_box, mem, sync::Arc, time::Instant};

#[derive(Deserialize, Serialize)]
#[serde(crate = "rt::serde", deny_unknown_fields)]
struct RustCreateUser {
    name: String,
    age: i32,
}

struct RustState {
    _ready: bool,
}

async fn rust_echo_wildcard(
    request: native_http::Request,
    _: Arc<RustState>,
) -> Result<native_http::Response, rt::Error> {
    let user: RustCreateUser = rt::decode(request.body())?;
    native_http::json(native_http::Status::OK, &user)
}

async fn rust_echo_named(
    mut request: native_http::Request,
    mut state: Arc<RustState>,
) -> Result<native_http::Response, rt::Error> {
    let mut user: RustCreateUser = rt::decode(&request.body())?;
    native_http::json(native_http::Status::OK, &user)
}

async fn generated_type_echo_wildcard(
    request: native_http::Request,
    _: Arc<State>,
) -> Result<native_http::Response, rt::Error> {
    let user: CreateUser = rt::decode(request.body())?;
    native_http::json(native_http::Status::OK, &user)
}

async fn generated_type_echo_named(
    mut request: native_http::Request,
    mut state: Arc<State>,
) -> Result<native_http::Response, rt::Error> {
    let mut user: CreateUser = rt::decode(&request.body())?;
    native_http::json(native_http::Status::OK, &user)
}

fn future_shape<S, H, F>(_: H) -> rt::serde_json::Value
where
    H: Fn(native_http::Request, Arc<S>) -> F,
    F: Future<Output = Result<native_http::Response, rt::Error>>,
{
    rt::serde_json::json!({"size_bytes": mem::size_of::<F>(), "align_bytes": mem::align_of::<F>()})
}

fn generated_json(input: &[u8]) -> Result<native_http::Response, rt::Error> {
    let mut user: CreateUser = rt::decode(&(input))?;
    native_http::json(native_http::Status::OK, &(user))
}

fn plain_json(input: &[u8]) -> Result<native_http::Response, rt::Error> {
    let user: RustCreateUser = rt::decode(input)?;
    native_http::json(native_http::Status::OK, &user)
}

fn sample(
    name: &str,
    round: usize,
    iterations: usize,
    input: &[u8],
    operation: impl Fn(&[u8]) -> Result<native_http::Response, rt::Error>,
) {
    let started = Instant::now();
    for _ in 0..iterations {
        black_box(operation(black_box(input)).unwrap());
    }
    let elapsed = started.elapsed();
    let (_, allocation) = rt::metrics::measure(|| {
        black_box(operation(black_box(input)).unwrap());
    });
    println!(
        "{}",
        rt::serde_json::json!({
            "phase": "decode_then_native_json_response",
            "implementation": name,
            "round": round,
            "iterations": iterations,
            "input_bytes": input.len(),
            "ns_per_operation": elapsed.as_nanos() as f64 / iterations as f64,
            "allocation": allocation,
            "allocation_scope": "one complete operation on calling thread; excludes input creation and HTTP"
        })
    );
}

fn main() {
    println!(
        "{}",
        rt::serde_json::json!({
            "phase": "type_and_future_layout",
            "request_size": mem::size_of::<native_http::Request>(),
            "generated_user_size": mem::size_of::<CreateUser>(),
            "rust_user_size": mem::size_of::<RustCreateUser>(),
            "generated_state_size": mem::size_of::<State>(),
            "rust_state_size": mem::size_of::<RustState>(),
            "generated_echo": future_shape::<State, _, _>(echo),
            "rust_echo_wildcard": future_shape::<RustState, _, _>(rust_echo_wildcard),
            "rust_echo_named": future_shape::<RustState, _, _>(rust_echo_named),
            "generated_type_wildcard": future_shape::<State, _, _>(generated_type_echo_wildcard),
            "generated_type_named": future_shape::<State, _, _>(generated_type_echo_named)
        })
    );
    let iterations = std::env::var("JSON_PROBE_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(10_000)
        .max(1);
    for length in [5, 4096] {
        // Preserve the name-before-age order used by benchmarks/http.lua.
        let input = format!(r#"{{"name":"{}","age":18}}"#, "x".repeat(length)).into_bytes();
        // Warm both monomorphizations before any timed or counted operation.
        for _ in 0..100 {
            black_box(generated_json(black_box(&input)).unwrap());
            black_box(plain_json(black_box(&input)).unwrap());
        }
        for round in 0..7 {
            if round % 2 == 0 {
                sample("generated", round, iterations, &input, generated_json);
                sample("plain_rust", round, iterations, &input, plain_json);
            } else {
                sample("plain_rust", round, iterations, &input, plain_json);
                sample("generated", round, iterations, &input, generated_json);
            }
        }
    }
}
