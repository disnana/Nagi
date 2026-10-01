use super::*;
use axum::{
    body::Body,
    response::Response,
    routing::{get, post},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    sync::oneshot,
};

const REQUEST: &[u8] = b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n";
const WAIT: Duration = Duration::from_millis(200);

struct Server {
    address: std::net::SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}
impl Server {
    async fn new(wait: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = Router::new()
            .route("/echo", post(|body: axum::body::Bytes| async move { body }))
            .route("/large", get(|| async { vec![b'x'; 32 * 1024 * 1024] }))
            .route(
                "/slow-stream",
                get(|| async {
                    let chunks = futures_util::stream::unfold(0, |index| async move {
                        if index == 6 {
                            None
                        } else {
                            tokio::time::sleep(Duration::from_millis(80)).await;
                            Some((Ok::<_, std::io::Error>("chunk\n"), index + 1))
                        }
                    });
                    Response::new(Body::from_stream(chunks))
                }),
            );
        let (shutdown, stopped) = oneshot::channel();
        let task = tokio::spawn(serve(listener, crate::http_router(router), wait, async {
            let _ = stopped.await;
        }));
        Self {
            address,
            shutdown: Some(shutdown),
            task,
        }
    }
    async fn connect(&self) -> BufReader<TcpStream> {
        BufReader::new(TcpStream::connect(self.address).await.unwrap())
    }
    async fn stop(mut self) {
        self.shutdown.take().unwrap().send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(3), &mut self.task)
            .await
            .unwrap()
            .unwrap();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.abort();
    }
}

async fn send(socket: &mut BufReader<TcpStream>, bytes: &[u8]) {
    socket.get_mut().write_all(bytes).await.unwrap();
}

async fn response(socket: &mut BufReader<TcpStream>) -> (u16, Vec<u8>) {
    let mut line = String::new();
    socket.read_line(&mut line).await.unwrap();
    let status = line.split_whitespace().nth(1).unwrap().parse().unwrap();
    let mut length = 0;
    let mut chunked = false;
    loop {
        line.clear();
        socket.read_line(&mut line).await.unwrap();
        if line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap();
            }
            if name.eq_ignore_ascii_case("transfer-encoding") {
                chunked = value.trim() == "chunked";
            }
        }
    }
    let mut body = vec![];
    if chunked {
        loop {
            line.clear();
            socket.read_line(&mut line).await.unwrap();
            let length = usize::from_str_radix(line.trim(), 16).unwrap();
            if length == 0 {
                socket.read_line(&mut line).await.unwrap();
                break;
            }
            let start = body.len();
            body.resize(start + length, 0);
            socket.read_exact(&mut body[start..]).await.unwrap();
            let mut end = [0; 2];
            socket.read_exact(&mut end).await.unwrap();
            assert_eq!(&end, b"\r\n");
        }
    } else {
        body.resize(length, 0);
        socket.read_exact(&mut body).await.unwrap();
    }
    (status, body)
}

async fn expired(socket: &mut BufReader<TcpStream>) {
    let mut remaining = vec![];
    tokio::time::timeout(Duration::from_secs(3), socket.read_to_end(&mut remaining))
        .await
        .unwrap()
        .unwrap();
    assert!(
        remaining.is_empty() || remaining.starts_with(b"HTTP/1.1 408"),
        "{remaining:?}"
    );
}

#[test]
fn request_wait_configuration_is_positive_finite_and_defaults_to_ten_seconds() {
    assert_eq!(parse_wait_timeout(None).unwrap(), Duration::from_secs(10));
    assert_eq!(
        parse_wait_timeout(Some("60")).unwrap(),
        Duration::from_secs(60)
    );
    for value in ["", "0", "-1", "1.5", "NaN", "ten", "18446744073709551615"] {
        assert!(parse_wait_timeout(Some(value)).is_err(), "{value}");
    }
}

#[tokio::test]
async fn silent_partial_and_dripped_headers_expire_without_extending_the_deadline() {
    let server = Server::new(WAIT).await;
    for prefix in [b"".as_slice(), b"GET /health HTTP/1.1\r\nHost:".as_slice()] {
        let mut socket = server.connect().await;
        send(&mut socket, prefix).await;
        let started = Instant::now();
        expired(&mut socket).await;
        assert!(started.elapsed() >= Duration::from_millis(120));
    }
    let mut socket = server.connect().await;
    send(&mut socket, b"GET /health HTTP/1.1\r\nHost:").await;
    for _ in 0..3 {
        tokio::time::sleep(Duration::from_millis(40)).await;
        send(&mut socket, b"x").await;
    }
    // At expiry the last byte is younger than the deadline. A timer reset on
    // every byte would keep this connection alive beyond this interval.
    tokio::time::timeout(Duration::from_millis(140), expired(&mut socket))
        .await
        .unwrap();
    server.stop().await;
}

#[tokio::test]
async fn idle_reuse_and_buffered_partial_requests_share_the_header_deadline() {
    let server = Server::new(WAIT).await;
    let mut socket = server.connect().await;
    for _ in 0..2 {
        send(&mut socket, REQUEST).await;
        assert_eq!(response(&mut socket).await, (200, b"ok".to_vec()));
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    expired(&mut socket).await;
    let mut socket = server.connect().await;
    send(&mut socket, REQUEST).await;
    assert_eq!(response(&mut socket).await.0, 200);
    send(&mut socket, b"GET /health HTTP/1.1\r\nHost:").await;
    expired(&mut socket).await;
    let mut socket = server.connect().await;
    send(
        &mut socket,
        &[REQUEST, b"GET /health HTTP/1.1\r\nHost:"].concat(),
    )
    .await;
    assert_eq!(response(&mut socket).await.0, 200);
    expired(&mut socket).await;
    server.stop().await;
}

#[tokio::test]
async fn streaming_and_backpressure_can_outlast_the_request_wait() {
    let server = Server::new(WAIT).await;
    let mut socket = server.connect().await;
    send(
        &mut socket,
        b"GET /slow-stream HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .await;
    assert_eq!(response(&mut socket).await, (200, b"chunk\n".repeat(6)));
    send(&mut socket, REQUEST).await;
    assert_eq!(response(&mut socket).await.0, 200);
    let mut socket = server.connect().await;
    send(
        &mut socket,
        b"GET /large HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .await;
    tokio::time::sleep(Duration::from_millis(450)).await;
    let (status, body) = response(&mut socket).await;
    assert_eq!(status, 200);
    assert_eq!(body, vec![b'x'; 32 * 1024 * 1024]);
    send(&mut socket, REQUEST).await;
    assert_eq!(response(&mut socket).await.0, 200);
    server.stop().await;
}

#[tokio::test]
async fn websocket_upgrade_is_exempt_from_the_http_wait() {
    let server = Server::new(WAIT).await;
    let mut socket = server.connect().await;
    send(&mut socket, b"GET /ws HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: MDEyMzQ1Njc4OWFiY2RlZg==\r\n\r\n").await;
    assert_eq!(response(&mut socket).await.0, 101);
    tokio::time::sleep(Duration::from_millis(450)).await;
    send(&mut socket, &[0x81, 0x82, 1, 2, 3, 4, b'h' ^ 1, b'i' ^ 2]).await;
    let mut frame = [0; 4];
    tokio::time::timeout(Duration::from_secs(2), socket.read_exact(&mut frame))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&frame, b"\x81\x02hi");
    drop(socket);
    server.stop().await;
}

#[tokio::test]
async fn body_timeout_remains_separate_from_header_wait() {
    let server = Server::new(WAIT).await;
    let mut socket = server.connect().await;
    send(
        &mut socket,
        b"POST /echo HTTP/1.1\r\nHost: localhost\r\nContent-Length: 10\r\n\r\nx",
    )
    .await;
    let started = Instant::now();
    assert_eq!(response(&mut socket).await.0, 408);
    assert!(started.elapsed() >= Duration::from_secs(1));
    server.stop().await;
}

#[tokio::test]
async fn healthy_requests_work_with_more_than_128_idle_connections_and_shutdown_closes_them() {
    let server = Server::new(Duration::from_secs(30)).await;
    let mut idle = vec![];
    for _ in 0..160 {
        idle.push(server.connect().await);
    }
    let mut socket = server.connect().await;
    send(&mut socket, REQUEST).await;
    assert_eq!(response(&mut socket).await, (200, b"ok".to_vec()));
    server.stop().await;
    for socket in &mut idle {
        expired(socket).await;
    }
}
