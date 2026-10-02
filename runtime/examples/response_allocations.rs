//! Compare the 0.1.5 response builders with the current builders in one process.
use axum::{
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use nagi_runtime::{axum, metrics, Error, ErrorKind};
use serde::Serialize;

fn baseline<T: Serialize>(r: Result<T, Error>) -> Response {
    match r {
        Ok(t) => match serde_json::to_vec(&t) {
            Ok(v) => ([(header::CONTENT_TYPE, "application/json")], v).into_response(),
            Err(e) => baseline_error(Error::internal(e.to_string())),
        },
        Err(e) => baseline_error(e),
    }
}
fn baseline_error(e: Error) -> Response {
    let code = match e.kind {
        ErrorKind::Invalid => StatusCode::BAD_REQUEST,
        ErrorKind::NotFound => StatusCode::NOT_FOUND,
        ErrorKind::Busy => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    #[derive(Serialize)]
    struct ErrorBody<'a> {
        error: &'a str,
    }
    let msg = if matches!(e.kind, ErrorKind::Database | ErrorKind::Internal) {
        eprintln!("{e}");
        "internal error"
    } else {
        &e.message
    };
    (
        code,
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::to_vec(&ErrorBody { error: msg }).unwrap(),
    )
        .into_response()
}
async fn wire(response: Response) -> (StatusCode, HeaderMap, Vec<u8>) {
    let (parts, body) = response.into_parts();
    (
        parts.status,
        parts.headers,
        axum::body::to_bytes(body, usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    #[derive(Serialize)]
    struct User<'a> {
        id: i64,
        name: &'a str,
        age: i32,
    }
    let mut rows = vec![];
    for size in [5, 4096] {
        let name = "x".repeat(size);
        let payload = User {
            id: 1,
            name: &name,
            age: 18,
        };
        let (old, before) = metrics::measure(|| baseline(Ok(&payload)));
        let (new, after) = metrics::measure(|| nagi_runtime::response(Ok(&payload)));
        assert_eq!(wire(old).await, wire(new).await);
        rows.push(serde_json::json!({"case": format!("json_name_{size}"), "baseline": before, "candidate": after}));
    }
    for (name, kind) in [
        ("invalid", ErrorKind::Invalid),
        ("not_found", ErrorKind::NotFound),
        ("busy", ErrorKind::Busy),
    ] {
        // Construct inputs before measuring either builder.
        let old_error = Error {
            kind,
            message: "invalid input 凪".into(),
        };
        let new_error = old_error.clone();
        let (old, before) = metrics::measure(|| baseline::<()>(Err(old_error)));
        let (new, after) = metrics::measure(|| nagi_runtime::response::<()>(Err(new_error)));
        assert_eq!(wire(old).await, wire(new).await);
        rows.push(serde_json::json!({"case": name, "baseline": before, "candidate": after}));
    }
    println!("{}", serde_json::to_string_pretty(&rows).unwrap());
}
