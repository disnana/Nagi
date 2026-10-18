//! Opt-in real-dispatcher calibration, independent of the two Session fixtures.
//! No SQLite operation is issued, so these observations cannot prove a DB wait.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::oneshot,
};

struct Server {
    address: std::net::SocketAddr,
    stopped: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<Result<(), Error>>,
}
impl Server {
    async fn new(app: App<(), Error>, options: Options) -> Self {
        let options = authority(
            options,
            "https://localhost",
            vec!["localhost".to_owned()],
            1,
            128,
        )
        .unwrap();
        // Keep this isolated fixture aligned with the existing 16 MiB
        // test-only stalled-send response profile.
        let body_bytes = options.body_bytes.max(16 * 1024 * 1024);
        let retained_bytes = body_bytes
            .checked_mul(8)
            .and_then(|bytes| bytes.checked_add(16 * 1024 * 1024))
            .expect("test response retained bound fits usize");
        let fallback_retained_bytes = options
            .connections
            .checked_mul(262_144)
            .and_then(|bytes| bytes.checked_add(16 * 1024 * 1024))
            .expect("test fallback retained bound fits usize");
        let limits = response_limits(
            i64::try_from(body_bytes).expect("test body bound fits i64"),
            65_536,
            256,
            4096,
            32,
            131_072,
            i64::try_from(options.requests).expect("test request count fits i64"),
            i64::try_from(retained_bytes).expect("test retained bound fits i64"),
            i64::try_from(options.connections).expect("test connection count fits i64"),
            i64::try_from(fallback_retained_bytes).expect("test fallback retained bound fits i64"),
            4096,
            32,
        )
        .expect("fixed test response limits are valid");
        let options =
            with_response_limits(options, limits).expect("fixed test response profile validates");
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stopped, shutdown) = oneshot::channel();
        let task = tokio::spawn(serve_listener(listener, app, options, async {
            let _ = shutdown.await;
        }));
        Self {
            address,
            stopped: Some(stopped),
            task,
        }
    }
    async fn status(&self, request: &[u8]) -> u16 {
        tokio::time::timeout(Duration::from_secs(3), async {
            let mut socket = TcpStream::connect(self.address).await.unwrap();
            socket.write_all(request).await.unwrap();
            let mut bytes = Vec::new();
            socket.take(4096).read_to_end(&mut bytes).await.unwrap();
            std::str::from_utf8(&bytes)
                .unwrap()
                .lines()
                .next()
                .unwrap()
                .split_whitespace()
                .nth(1)
                .unwrap()
                .parse()
                .unwrap()
        })
        .await
        .unwrap()
    }
    async fn stop(mut self) {
        self.stopped.take().unwrap().send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(3), &mut self.task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(stopped) = self.stopped.take() {
            let _ = stopped.send(());
        }
        self.task.abort();
    }
}

#[derive(Clone, Copy)]
enum Mode {
    PreparationTimer,
    PreparationLateReady,
    HandlerTimer,
    HandlerLateReady,
    Ok200,
    Invalid400,
    Unauthorized401,
    Panic500,
    Body408,
}

async fn run(mode: Mode, expected: u16) {
    let preparation_completed = Arc::new(AtomicBool::new(false));
    let handler_reached = Arc::new(AtomicBool::new(false));
    let prepared_flag = Arc::clone(&preparation_completed);
    let reached_flag = Arc::clone(&handler_reached);
    let prepare: RoutePrepare<()> = Box::new(move |_, _, _| {
        let prepared_flag = Arc::clone(&prepared_flag);
        let reached_flag = Arc::clone(&reached_flag);
        Box::pin(async move {
            match mode {
                Mode::PreparationTimer => {
                    // Retain the sender until cancellation; the first poll is
                    // deterministically Pending, with no background worker.
                    let (held, waiting) = oneshot::channel::<()>();
                    let _ = waiting.await;
                    drop(held);
                    panic!("preparation timer calibration resumed");
                }
                Mode::PreparationLateReady => {
                    // Deliberately non-yielding first poll exceeds the unchanged
                    // default 2s preparation bound. This is not a production delay.
                    std::thread::sleep(Duration::from_millis(2100));
                }
                _ => {}
            }
            prepared_flag.store(true, Ordering::SeqCst);
            let handler: PreparedHandler<()> = Box::new(move |request, _| {
                Box::pin(async move {
                    assert!(prepared_flag.load(Ordering::SeqCst));
                    reached_flag.store(true, Ordering::SeqCst);
                    match mode {
                        Mode::HandlerTimer => {
                            let (held, waiting) = oneshot::channel::<()>();
                            let _ = waiting.await;
                            drop(held);
                            panic!("handler timer calibration resumed");
                        }
                        Mode::HandlerLateReady => {
                            // One Ready poll deliberately exceeds its own 20ms
                            // handler bound; no clock or deadline is substituted.
                            std::thread::sleep(Duration::from_millis(40));
                        }
                        Mode::Panic500 => panic!("private calibration panic"),
                        _ => {}
                    }
                    let status = match mode {
                        Mode::Invalid400 => Status::BAD_REQUEST,
                        Mode::Unauthorized401 => Status::UNAUTHORIZED,
                        _ => Status::OK,
                    };
                    match text(response_context(&request), status, "calibration") {
                        Ok(response) => PreparedCompletion::HandlerOk(response),
                        Err(error) => PreparedCompletion::MapperFailure(error),
                    }
                })
            });
            Ok(handler)
        })
    });
    let mut app = app_default(());
    app.paths.insert("/", "/".to_owned()).unwrap();
    app.routes.insert(
        "/".to_owned(),
        vec![Route {
            source: PolicySource::Public,
            method: Method::GET,
            kind: RouteKind::Ordinary(prepare),
            cors: None,
        }],
    );
    // The two original private tests retain their own 2s/20ms options untouched.
    // This separate fixture uses the same finite default preparation bound and
    // 20ms handler bound, plus a 1s body deadline for the independent408 control.
    let server = Server::new(app, options(4, 1000, 20, 1000).unwrap()).await;
    let request: &[u8] = if matches!(mode, Mode::Body408) {
        b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 1\r\n\r\n"
    } else {
        b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
    };
    let received = server.status(request).await;
    // Always complete the actual owned server join before checking the status.
    server.stop().await;
    assert_eq!(received, expected);
    let preparation = preparation_completed.load(Ordering::SeqCst);
    let reached = handler_reached.load(Ordering::SeqCst);
    match mode {
        Mode::PreparationTimer => assert!(!preparation && !reached),
        Mode::PreparationLateReady | Mode::Body408 => assert!(preparation && !reached),
        _ => assert!(preparation && reached),
    }
}

#[tokio::test]
async fn preparation_timer() {
    run(Mode::PreparationTimer, 504).await;
}
#[tokio::test]
async fn preparation_late_ready() {
    run(Mode::PreparationLateReady, 504).await;
}
#[tokio::test]
async fn handler_timer() {
    run(Mode::HandlerTimer, 504).await;
}
#[tokio::test]
async fn handler_late_ready() {
    run(Mode::HandlerLateReady, 504).await;
}
#[tokio::test]
async fn positive_200() {
    run(Mode::Ok200, 200).await;
}
#[tokio::test]
async fn positive_400() {
    run(Mode::Invalid400, 400).await;
}
#[tokio::test]
async fn positive_401() {
    run(Mode::Unauthorized401, 401).await;
}
#[tokio::test]
async fn positive_500() {
    run(Mode::Panic500, 500).await;
}
#[tokio::test]
async fn body_408_is_separate() {
    run(Mode::Body408, 408).await;
}
#[tokio::test]
async fn concurrent_request_owners() {
    tokio::join!(
        run(Mode::PreparationTimer, 504),
        run(Mode::HandlerTimer, 504)
    );
}
