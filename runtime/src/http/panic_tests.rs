use super::*;
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::oneshot,
};

fn construction_panic() -> std::future::Ready<&'static str> {
    panic!("private constructor detail")
}

struct PanickingExtractor;

impl FromRequestParts<()> for PanickingExtractor {
    type Rejection = StatusCode;

    async fn from_request_parts(_: &mut Parts, _: &()) -> Result<Self, Self::Rejection> {
        tokio::task::yield_now().await;
        panic!("private extractor detail")
    }
}

struct PanickingResponse;

impl IntoResponse for PanickingResponse {
    fn into_response(self) -> Response {
        panic!("private response conversion detail")
    }
}

async fn exchange(address: std::net::SocketAddr, method: &str, path: &str) -> Vec<u8> {
    let mut socket = TcpStream::connect(address).await.unwrap();
    let connection = if path == "/health" {
        "Connection: close\r\n"
    } else {
        ""
    };
    socket
        .write_all(
            format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\n{connection}\r\n").as_bytes(),
        )
        .await
        .unwrap();
    let mut response = vec![];
    tokio::time::timeout(Duration::from_secs(3), socket.read_to_end(&mut response))
        .await
        .expect("the response connection must close")
        .unwrap();
    response
}

#[tokio::test]
async fn handler_extractor_and_response_panics_return_sanitized_500_for_get_and_head() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new()
        .route("/constructor", get(construction_panic))
        .route(
            "/poll",
            get(|| async {
                tokio::task::yield_now().await;
                panic!("private polled handler detail");
                #[allow(unreachable_code)]
                "unreachable"
            }),
        )
        .route(
            "/divide",
            get(|| async { axum::Json(10 / std::hint::black_box(0)) }),
        )
        .route(
            "/index",
            get(|| async {
                let values = [1, 2];
                axum::Json(values[std::hint::black_box(2)])
            }),
        )
        .route(
            "/extractor",
            get(|_: PanickingExtractor| async { "unreachable" }),
        )
        .route("/response", get(|| async { PanickingResponse }));
    let (shutdown, stopped) = oneshot::channel();
    let task = tokio::spawn(serve(
        listener,
        crate::http_router(router),
        Duration::from_secs(10),
        async {
            let _ = stopped.await;
        },
    ));
    for path in [
        "/constructor",
        "/poll",
        "/divide",
        "/index",
        "/extractor",
        "/response",
    ] {
        for method in ["GET", "HEAD"] {
            let response = exchange(address, method, path).await;
            let response = std::str::from_utf8(&response).unwrap();
            let (headers, body) = response.split_once("\r\n\r\n").unwrap_or_else(|| {
                panic!("{method} {path} did not return a complete HTTP response: {response:?}")
            });
            assert!(headers.starts_with("HTTP/1.1 500"), "{response}");
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains("\r\nconnection: close"),
                "{headers}"
            );
            if method == "HEAD" {
                assert!(
                    body.is_empty(),
                    "HEAD must not send response bytes: {body:?}"
                );
                assert!(
                    headers
                        .to_ascii_lowercase()
                        .contains("\r\ncontent-length: 21"),
                    "{headers}"
                );
            } else {
                assert_eq!(body, "Internal Server Error");
            }
            assert!(
                !response.contains("private"),
                "panic details must not enter the response"
            );
            let health = exchange(address, "GET", "/health").await;
            assert!(health.starts_with(b"HTTP/1.1 200"), "{health:?}");
            assert!(health.ends_with(b"ok"), "{health:?}");
            assert!(
                !task.is_finished(),
                "another request must still be accepted"
            );
        }
    }
    shutdown.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
}
