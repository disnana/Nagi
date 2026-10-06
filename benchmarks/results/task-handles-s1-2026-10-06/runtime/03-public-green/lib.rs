pub use axum;
pub use rusqlite;
pub use serde;
pub use serde_json;
pub mod actor;
pub mod auth;
mod concurrent;
mod database;
mod http;
pub mod http_server;
pub mod metrics;
pub mod result;
#[cfg(test)]
mod sqlite_prototype;
mod task;
use axum::{
    body::Body,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
pub use concurrent::*;
pub use database::indices as database_indices;
pub use database::{Db, FromRow, Sql};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    time::{Duration, Instant},
};
pub use task::{Task, TaskFailure, TaskFailureKind, TaskScope};

#[derive(Debug, Clone, Copy)]
pub enum ErrorKind {
    Invalid,
    NotFound,
    Busy,
    Database,
    Internal,
}
#[derive(Debug, Clone)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
}
impl Error {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Invalid,
            message: message.into(),
        }
    }
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Internal,
            message: message.into(),
        }
    }
    pub fn not_found() -> Self {
        Self {
            kind: ErrorKind::NotFound,
            message: "not found".into(),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}
impl std::error::Error for Error {}
/// Stable names for branching and diagnostics from Nagi.
pub fn error_kind(error: &Error) -> &'static str {
    match error.kind {
        ErrorKind::Invalid => "invalid",
        ErrorKind::NotFound => "not_found",
        ErrorKind::Busy => "busy",
        ErrorKind::Database => "database",
        ErrorKind::Internal => "internal",
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::invalid(e.to_string())
    }
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Self {
            kind: ErrorKind::Database,
            message: e.to_string(),
        }
    }
}
pub fn decode<'a, T: Deserialize<'a>>(input: &'a [u8]) -> Result<T, Error> {
    Ok(serde_json::from_slice(input)?)
}
pub fn encode<T: Serialize>(value: &T) -> Result<String, Error> {
    Ok(serde_json::to_string(value)?)
}
pub fn parse_i64(s: &str) -> Result<i64, Error> {
    s.parse()
        .map_err(|e: std::num::ParseIntError| Error::invalid(e.to_string()))
}
// Console-only blocking input. Flush prompts before waiting; preserve spaces.
pub fn read_line() -> Result<String, Error> {
    use std::io::Write;
    std::io::stdout()
        .flush()
        .map_err(|e| Error::internal(e.to_string()))?;
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| Error::internal(e.to_string()))?;
    Ok(line.trim_end_matches(['\r', '\n']).to_owned())
}
pub fn parse_f64(s: &str) -> Result<f64, Error> {
    let x: f64 = s
        .parse()
        .map_err(|e: std::num::ParseFloatError| Error::invalid(e.to_string()))?;
    if x.is_finite() {
        Ok(x)
    } else {
        Err(Error::invalid("finite float required"))
    }
}
pub fn slice<T>(input: &[T], start: i64, end: i64) -> Result<&[T], Error> {
    let a = usize::try_from(start).map_err(|_| Error::invalid("negative slice start"))?;
    let b = usize::try_from(end).map_err(|_| Error::invalid("negative slice end"))?;
    input
        .get(a..b)
        .ok_or_else(|| Error::invalid("slice out of bounds"))
}
pub fn slice_str(input: &str, start: i64, end: i64) -> Result<&str, Error> {
    let a = usize::try_from(start).map_err(|_| Error::invalid("negative slice start"))?;
    let b = usize::try_from(end).map_err(|_| Error::invalid("negative slice end"))?;
    input
        .get(a..b)
        .ok_or_else(|| Error::invalid("slice out of bounds or UTF-8 boundary"))
}
pub async fn sleep(ms: i64) {
    tokio::time::sleep(Duration::from_millis(ms.max(0) as u64)).await;
}
pub fn clock_ns() -> i64 {
    use std::sync::OnceLock;
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    ORIGIN.get_or_init(Instant::now).elapsed().as_nanos() as i64
}
pub fn make_ints(n: i64) -> Vec<i64> {
    (0..n.max(0)).map(|i| (i * 17 + 13) % 997 - 498).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timestamp(pub i64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Uuid(pub u128);
impl Uuid {
    pub fn parse(s: &str) -> Result<Self, Error> {
        let mut value = 0u128;
        let mut count = 0;
        for (i, c) in s.chars().enumerate() {
            if [8, 13, 18, 23].contains(&i) {
                if c != '-' {
                    return Err(Error::invalid("UUID separator"));
                }
                continue;
            }
            let n = c.to_digit(16).ok_or_else(|| Error::invalid("UUID hex"))?;
            value = value
                .checked_mul(16)
                .and_then(|v| v.checked_add(n as u128))
                .ok_or_else(|| Error::invalid("UUID overflow"))?;
            count += 1;
        }
        if s.len() != 36 || count != 32 {
            return Err(Error::invalid("UUID requires 36 ASCII chars"));
        }
        Ok(Self(value))
    }
}
impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{:032x}", self.0);
        write!(
            f,
            "{}-{}-{}-{}-{}",
            &s[..8],
            &s[8..12],
            &s[12..16],
            &s[16..20],
            &s[20..]
        )
    }
}

pub fn response<T: Serialize>(r: Result<T, Error>) -> Response {
    match r {
        Ok(t) => match serde_json::to_vec(&t) {
            Ok(v) => (
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                )],
                v,
            )
                .into_response(),
            Err(e) => error_response(Error::internal(e.to_string())),
        },
        Err(e) => error_response(e),
    }
}
pub fn error_response(e: Error) -> Response {
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
    // DB/内部エラーの詳細をHTTPへ出さない。ログには診断用の内容を残す。
    let msg = if matches!(e.kind, ErrorKind::Database | ErrorKind::Internal) {
        eprintln!("{e}");
        "internal error"
    } else {
        &e.message
    };
    (
        code,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )],
        serde_json::to_vec(&ErrorBody { error: msg }).unwrap(),
    )
        .into_response()
}
fn http_router(router: Router) -> Router {
    router
        .route("/health", axum::routing::get(|| async { "ok" }))
        .route(
            "/stream",
            axum::routing::get(|| async {
                let s = futures_util::stream::iter(
                    (0..5).map(|i| Ok::<_, std::io::Error>(format!("chunk:{i}\n"))),
                );
                Response::new(Body::from_stream(s))
            }),
        )
        .route("/ws", axum::routing::get(ws_handler))
        .layer(axum::extract::DefaultBodyLimit::max(1_048_576))
        .layer(axum::middleware::from_fn(
            |req: axum::extract::Request, next: axum::middleware::Next| async move {
                match tokio::time::timeout(Duration::from_secs(2), next.run(req)).await {
                    Ok(r) => r,
                    Err(_) => (StatusCode::REQUEST_TIMEOUT, "timeout").into_response(),
                }
            },
        ))
}
pub async fn serve(router: Router, port: i64) -> Result<(), Error> {
    let port = u16::try_from(port).map_err(|_| Error::invalid("port out of range"))?;
    let request_wait = http::request_wait_timeout()?;
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .map_err(|e| Error::internal(e.to_string()))?;
    println!("Nagi listening http://127.0.0.1:{port}");
    http::serve(listener, http_router(router), request_wait, async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await;
    Ok(())
}
async fn ws_handler(ws: axum::extract::ws::WebSocketUpgrade) -> Response {
    ws.max_message_size(1_048_576)
        .on_upgrade(|mut socket| async move {
            use axum::extract::ws::Message;
            while let Some(Ok(msg)) = socket.recv().await {
                match msg {
                    Message::Close(_) => break,
                    Message::Text(t) => {
                        if socket.send(Message::Text(t)).await.is_err() {
                            break;
                        }
                    }
                    Message::Binary(b) => {
                        if socket.send(Message::Binary(b)).await.is_err() {
                            break;
                        }
                    }
                    Message::Ping(p) => {
                        if socket.send(Message::Pong(p)).await.is_err() {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        })
}

pub fn bench_i64(name: &str, n: i64, f: fn(&[i64]) -> i64) {
    let data = make_ints(n);
    println!(
        "{}",
        serde_json::json!({"name":format!("{name}_checksum"),"checksum":f(&data)})
    );
    metrics::benchmark(name, n as usize, || {
        std::hint::black_box(f(std::hint::black_box(&data)))
    });
}
pub fn bench_f64(name: &str, n: i64, f: fn(&[f64]) -> f64) {
    let data: Vec<f64> = make_ints(n).iter().map(|&x| x as f64 / 7.0).collect();
    println!(
        "{}",
        serde_json::json!({"name":format!("{name}_checksum"),"checksum":f(&data)})
    );
    metrics::benchmark(name, n as usize, || {
        std::hint::black_box(f(std::hint::black_box(&data)))
    });
}
pub fn bench_scalar(name: &str, n: i64, f: fn(i64) -> i64) {
    println!(
        "{}",
        serde_json::json!({"name":format!("{name}_checksum"),"checksum":f(n)})
    );
    metrics::benchmark(name, n as usize, || {
        std::hint::black_box(f(std::hint::black_box(n)))
    });
}

pub fn block_on<F: std::future::Future>(f: F) -> F::Output {
    let workers = std::env::var("NAGI_THREADS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(4)
        .clamp(1, 32);
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .build()
        .expect("runtime creation")
        .block_on(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct User {
        id: i64,
        name: String,
    }
    #[test]
    fn json_direct() {
        let u: User = decode(br#"{"id":1,"name":"tp"}"#).unwrap();
        assert_eq!(u.name, "tp");
        assert_eq!(encode(&u).unwrap(), r#"{"id":1,"name":"tp"}"#);
    }
    #[test]
    fn json_wrong_type() {
        assert!(decode::<User>(br#"{"id":"1","name":"tp"}"#).is_err());
        assert!(decode::<User>(br#"{"id":1,"name":"tp","x":2}"#).is_err());
    }
    #[test]
    fn json_invalid_utf8() {
        assert!(decode::<User>(&[b'{', 0xff, b'}']).is_err());
    }
    #[test]
    fn zero_copy_pointer() {
        let x = vec![1u8, 2, 3, 4];
        let (v, m) = metrics::measure(|| slice(&x, 1, 3).unwrap());
        assert_eq!(v.as_ptr(), unsafe { x.as_ptr().add(1) });
        assert_eq!(m.allocations, 0);
        assert_eq!(v, &[2, 3]);
    }
    #[test]
    fn slice_bounds() {
        assert!(slice(&[1, 2], -1, 1).is_err());
        assert!(slice(&[1, 2], 0, 3).is_err());
        assert!(slice(&[1, 2], 2, 1).is_err());
    }
    #[test]
    fn uuid_roundtrip() {
        let s = "12345678-1234-5678-9abc-123456789def";
        assert_eq!(Uuid::parse(s).unwrap().to_string(), s);
        assert!(Uuid::parse("x").is_err());
        assert_eq!(std::mem::size_of::<Uuid>(), 16);
    }
    #[test]
    fn float_validation() {
        assert!(parse_f64("NaN").is_err());
        assert!(parse_f64("inf").is_err());
        assert_eq!(parse_f64("1.25").unwrap(), 1.25);
    }
    #[test]
    fn arena_scope() {
        let arena = bumpalo::Bump::new();
        let slice = arena.alloc_slice_copy(&[1u64, 2, 3]);
        assert_eq!(slice.iter().sum::<u64>(), 6);
    }
    #[test]
    fn borrowed_json_address() {
        #[derive(Deserialize)]
        struct B<'a> {
            name: &'a str,
        }
        let input = br#"{"name":"alice"}"#;
        let b: B = decode(input).unwrap();
        let addr = b.name.as_ptr() as usize;
        assert!(
            addr >= input.as_ptr() as usize
                && addr + b.name.len() <= input.as_ptr() as usize + input.len()
        );
        assert!(decode::<B>(br#"{"name":"tp\u002dli"}"#).is_err());
    }
}
