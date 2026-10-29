//! Case-local ready-pool experiment calibration; actual guarded HTTP, unchanged 1s budget.
use super::*;
use std::sync::Mutex;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::oneshot,
};

struct StartupState {
    store: session::Store,
    calls: AtomicUsize,
    window: Mutex<Option<(Instant, Arc<Lease>)>>,
    held_responses: Mutex<Vec<Response>>,
}
struct StartupServer {
    address: std::net::SocketAddr,
    stopped: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<Result<(), Error>>,
}
impl StartupServer {
    async fn new(
        store: &session::Store,
        csrf: &csrf::CsrfStore,
        options: Options,
    ) -> (Self, Arc<StartupState>) {
        let mut app = route(
            app_default(StartupState {
                store: session::clone_store(store),
                calls: AtomicUsize::new(0),
                window: Mutex::new(None),
                held_responses: Mutex::new(Vec::new()),
            }),
            Method::POST,
            "/issue",
            authenticated_policy(|_, _: Arc<StartupState>| async {
                VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(60))
            }),
            |request, state: Arc<StartupState>, scope| async move {
                let intent = session::issue(&state.store, scope)
                    .await
                    .map_err(|_| Error::internal("setup issue refused"))?;
                session::apply(empty(response_context(&request), Status::OK)?, intent)
                    .map_err(|_| Error::internal("setup delivery refused"))
            },
        )
        .unwrap();
        app = session_csrf_tokens(app, "/csrf", csrf::clone_store(csrf)).unwrap();
        let policy = session_csrf_policy(
            session_authenticated_policy(session::clone_store(store)),
            csrf::clone_store(csrf),
        )
        .unwrap();
        app = route(
            app,
            Method::POST,
            "/write",
            policy,
            |request, state: Arc<StartupState>, _: AuthScope| async move {
                state.calls.fetch_add(1, Ordering::SeqCst);
                empty(response_context(&request), Status::OK)
            },
        )
        .unwrap();
        let busy_policy = session_csrf_policy(
            session_authenticated_policy(session::clone_store(store)),
            csrf::clone_store(csrf),
        )
        .unwrap();
        app = route(
            app,
            Method::POST,
            "/write-busy",
            busy_policy,
            |request, state: Arc<StartupState>, _: AuthScope| async move {
                state.calls.fetch_add(1, Ordering::SeqCst);
                let context = response_context(&request);
                loop {
                    match text(context, Status::OK, "held") {
                        Ok(response) => state.held_responses.lock().unwrap().push(response),
                        Err(error) => break Err(error),
                    }
                }
            },
        )
        .unwrap();
        // Observe the production-constrained original lease before its real
        // Session lookup. This wrapper delegates unchanged and adds no await.
        for path in ["/write", "/write-busy"] {
            let routes = app.routes.remove(path).unwrap();
            let mut wrapped = Vec::new();
            for route in routes {
                let RouteKind::Guarded { store, prepare } = route.kind else {
                    panic!("guarded fixture route required")
                };
                let prepare: GuardedPrepare<StartupState> =
                    Box::new(move |request, state, lease| {
                        let until = csrf::live_deadline(&lease).unwrap();
                        #[cfg(nagi_auth_stage_diag)]
                        {
                            let observer =
                                crate::auth::session_deadline_observer::current().unwrap();
                            let (admitted, original) = observer.admission_window_for_test();
                            assert_eq!(original, admitted + Duration::from_millis(1000));
                            assert_eq!(
                                until, original,
                                "startup uses original admitted security deadline"
                            );
                        }
                        let previous = state
                            .window
                            .lock()
                            .unwrap()
                            .replace((until, Arc::clone(&lease)));
                        assert!(
                            previous.is_none(),
                            "one guarded request per independent arm"
                        );
                        prepare(request, state, lease)
                    });
                wrapped.push(Route {
                    kind: RouteKind::Guarded { store, prepare },
                    ..route
                });
            }
            app.routes.insert(path.to_owned(), wrapped);
        }
        let state = Arc::clone(&app.state);
        let (app, options) = crate::http_server::tests::with_rate_fixture(app, options);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stopped, shutdown) = oneshot::channel();
        let task = tokio::spawn(serve_listener(listener, app, options, async {
            let _ = shutdown.await;
        }));
        (
            Self {
                address,
                stopped: Some(stopped),
                task,
            },
            state,
        )
    }
    async fn request(&self, method: &str, path: &str, headers: &str) -> (u16, String, String) {
        tokio::time::timeout(Duration::from_secs(3), async {
            let mut socket = TcpStream::connect(self.address).await.unwrap();
            let request = format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 0\r\n{headers}\r\n");
            socket.write_all(request.as_bytes()).await.unwrap();
            let mut bytes = Vec::new();
            socket.take(8192).read_to_end(&mut bytes).await.unwrap();
            let wire = std::str::from_utf8(&bytes).unwrap();
            let (head, body) = wire.split_once("\r\n\r\n").unwrap();
            let status = head.lines().next().unwrap().split_whitespace().nth(1).unwrap().parse().unwrap();
            let cookie = head.lines().filter_map(|line| line.split_once(':'))
                .find(|(name, _)| name.eq_ignore_ascii_case("set-cookie"))
                .map(|(_, value)| value.trim().split(';').next().unwrap().to_owned()).unwrap_or_default();
            (status, cookie, body.to_owned())
        }).await.expect("HTTP startup calibration watchdog; never a successful timeout oracle")
    }
    // Setup must not spend the worker2 gate reserved for the /write control.
    // Observe it without granting a new deadline or accepting a delayed setup.
    async fn bootstrap_request(
        &self,
        startup: &sqlite::HttpStartupTest,
        pool: &sqlite::Pool,
        stage: &'static str,
        method: &str,
        path: &str,
        headers: &str,
    ) -> (u16, String, String) {
        let started = Instant::now();
        let response = self.request(method, path, headers);
        tokio::pin!(response);
        let response = tokio::select! {
            biased;
            _ = startup.wait() => {
                let created = startup.created();
                let idle = startup.idle(pool);
                eprintln!(
                    "b1_startup_bootstrap_gate stage={stage} created={created} idle={idle} elapsed_us={}",
                    started.elapsed().as_micros(),
                );
                startup.release();
                let (status, _, _) = response.await;
                panic!(
                    "bootstrap worker2 gate: stage={stage} created={created} idle={idle} drained_status={status} elapsed_us={}",
                    started.elapsed().as_micros(),
                );
            }
            response = &mut response => response,
        };
        let created = startup.created();
        let idle = startup.idle(pool);
        eprintln!(
            "b1_startup_bootstrap stage={stage} status={} created={created} idle={idle} elapsed_us={}",
            response.0,
            started.elapsed().as_micros(),
        );
        assert_eq!(
            created, 1,
            "bootstrap worker2 selection: stage={stage} idle={idle}"
        );
        response
    }
    async fn stop(mut self) -> bool {
        if let Some(stopped) = self.stopped.take() {
            let _ = stopped.send(());
        }
        matches!(
            tokio::time::timeout(Duration::from_secs(3), &mut self.task).await,
            Ok(Ok(Ok(())))
        )
    }
}
impl Drop for StartupServer {
    fn drop(&mut self) {
        if let Some(stopped) = self.stopped.take() {
            let _ = stopped.send(());
        }
        self.task.abort();
    }
}
struct TokenCount(i64);
impl crate::FromRow for TokenCount {
    fn columns() -> &'static [&'static str] {
        &["n"]
    }
    fn read(row: &crate::rusqlite::Row<'_>, indices: &[usize]) -> crate::rusqlite::Result<Self> {
        Ok(Self(row.get(indices[0])?))
    }
}
async fn startup_case(ready: bool) {
    let dir = loop {
        let p = std::env::temp_dir().join(format!(
            "csrf-startup-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match std::fs::create_dir(&p) {
            Ok(()) => break p,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => panic!("owned startup directory unavailable"),
        }
    };
    let (pool, startup) = sqlite::http_startup_test_pool(dir.join("owned.sqlite"));
    let mut server = None;
    let mut held = None;
    let observed = AssertUnwindSafe(async {
        let options = authority(security_timeout(options(4, 1000, 1000, 1000).unwrap(), 1000).unwrap(),
            "https://localhost", vec!["localhost".to_owned()], 1, 128).unwrap();
        let options = crate::http_server::tests::with_response_fixture(options);
        let store = session::open(&pool,
            session::options(4, 5, 72, 1, 10000, 100000, 5000, 2).unwrap(),
            session::cookie_options("__Host-auth", session::SameSite::Strict).unwrap()).await.unwrap();
        let csrf = csrf::open(&store, csrf::options(2, 176, 1, 10000, 2).unwrap(), &options).await.unwrap();
        let (running, state) = StartupServer::new(&store, &csrf, options).await;
        server = Some(running);
        let running = server.as_ref().unwrap();
        let (issued, cookie, _) = running.bootstrap_request(&startup, &pool, "issue", "POST", "/issue", "Authorization: Bearer owned\r\n").await;
        assert_eq!(issued, 200);
        assert!(!cookie.is_empty());
        let (delivered, _, body) = running.bootstrap_request(&startup, &pool, "csrf", "GET", "/csrf", &format!("Cookie: {cookie}\r\n")).await;
        assert_eq!(delivered, 200);
        let json: crate::serde_json::Value = crate::serde_json::from_str(&body).unwrap();
        let token = json["csrf_token"].as_str().unwrap();
        assert_eq!(token.len(), 43);
        assert_eq!(startup.created(), 1, "sequential bootstrap warmed only worker1");
        if ready {
            held = Some(sqlite::begin(&pool, sqlite::BeginMode::Deferred).await.unwrap());
            let second = sqlite::begin(&pool, sqlite::BeginMode::Deferred);
            tokio::pin!(second);
            tokio::select! {
                _ = startup.wait() => {},
                _ = &mut second => panic!("worker2 cannot become ready before startup gate release"),
            }
            startup.release();
            let second = second.await.unwrap();
            sqlite::rollback(held.take().unwrap()).await.unwrap();
            sqlite::rollback(second).await.unwrap();
            assert_eq!(startup.created(), 2);
            assert_eq!(startup.idle(&pool), 2, "both actual checkouts returned before HTTP admission");
        }
        held = Some(sqlite::begin(&pool, sqlite::BeginMode::Deferred).await.unwrap());
        let headers = format!("Cookie: {cookie}\r\nOrigin: https://localhost\r\nX-CSRF-Token: {token}\r\n");
        let response = running.request("POST", "/write", &headers);
        tokio::pin!(response);
        if !ready {
            tokio::select! {
                _ = startup.wait() => {},
                _ = &mut response => panic!("cold request must reach the closed worker2 startup gate"),
            }
            let (until, lease) = {
                let window = state.window.lock().unwrap();
                let (until, lease) = window.as_ref().expect("actual guarded original deadline captured before checkout");
                (*until, Arc::clone(lease))
            };
            // Observe original lease expiry with startup still blocked. Release
            // native work before awaiting HTTP delivery; receipt latency and
            // native join timing are not success guarantees of this control.
            tokio::time::sleep_until(tokio::time::Instant::from_std(until)).await;
            assert!(Instant::now() >= until);
            assert!(lease.validate().is_err());
            assert_eq!(state.calls.load(Ordering::SeqCst), 0);
            startup.release();
        }
        let (status, _, _) = response.await;
        assert_eq!(status, if ready { 200 } else { 504 });
        assert_eq!(state.calls.load(Ordering::SeqCst), usize::from(ready));
        {
            let window = state.window.lock().unwrap();
            let (until, lease) = window.as_ref().expect("actual guarded preparation selected original lease");
            if !ready { assert!(Instant::now() >= *until); assert!(lease.validate().is_err()); }
        }
        assert_eq!(startup.created(), 2, "request cannot require a third worker");
        startup.release();
        sqlite::rollback(held.take().unwrap()).await.unwrap();
        let tx = sqlite::begin(&pool, sqlite::BeginMode::Deferred).await.unwrap();
        let count = sqlite::query::<TokenCount>(&tx, sqlite::literal("SELECT count(*) AS n FROM __nagi_csrf_tokens"), sqlite::parameters()).await.unwrap().unwrap().0;
        sqlite::rollback(tx).await.unwrap();
        assert_eq!(count, 1, "cold refusal and ready success preserve the issued reusable token row");
    }).catch_unwind().await;
    // Every assertion/watchdog path releases native startup, drops request
    // futures, returns the reserved checkout and observes actual close/join.
    startup.release();
    let rollback_ok = match held.take() {
        Some(tx) => sqlite::rollback(tx).await.is_ok(),
        None => true,
    };
    let server_ok = match server.take() {
        Some(running) => running.stop().await,
        None => true,
    };
    let close_ok = sqlite::close(&pool, 3000).await.is_ok();
    let joined = startup.all_joined();
    // Retain the owned database if any cleanup/join is still unconfirmed.
    let removed =
        rollback_ok && server_ok && close_ok && joined && std::fs::remove_dir_all(dir).is_ok();
    assert!(
        rollback_ok && server_ok && close_ok && joined && removed,
        "startup fixture cleanup must actually join every owned worker"
    );
    if let Err(panic) = observed {
        std::panic::resume_unwind(panic);
    }
}
#[tokio::test]
async fn cold_worker2_startup_spends_original_guarded_http_budget() {
    startup_case(false).await;
}
#[tokio::test]
async fn ready_worker2_setup_preserves_same_guarded_http_budget() {
    startup_case(true).await;
}

#[tokio::test]
async fn guarded_busy_mapper_returns_503_retires_owner_and_recovers_capacity() {
    let f = Fixture::new().await;
    let options = authority(
        security_timeout(options(4, 1000, 1000, 1000).unwrap(), 1000).unwrap(),
        "https://localhost",
        vec!["localhost".to_owned()],
        1,
        128,
    )
    .unwrap();
    let options = crate::http_server::tests::with_response_fixture(options);
    let csrf = csrf::open(
        &f.store,
        csrf::options(2, 176, 1, 10_000, 2).unwrap(),
        &options,
    )
    .await
    .unwrap();
    let (running, state) = StartupServer::new(&f.store, &csrf, options).await;
    let (issued, cookie, _) = running
        .request("POST", "/issue", "Authorization: Bearer owned\r\n")
        .await;
    assert_eq!(issued, 200);
    assert!(!cookie.is_empty());
    let (status, _, body) = running
        .request("GET", "/csrf", &format!("Cookie: {cookie}\r\n"))
        .await;
    assert_eq!(status, 200);
    let json: crate::serde_json::Value = crate::serde_json::from_str(&body).unwrap();
    let token = json["csrf_token"].as_str().unwrap();
    let headers =
        format!("Cookie: {cookie}\r\nOrigin: https://localhost\r\nX-CSRF-Token: {token}\r\n");

    let (status, _, body) = running.request("POST", "/write-busy", &headers).await;
    assert_eq!(status, 503);
    assert_eq!(body, "service unavailable");
    assert!(!body.contains("response capacity"));
    let lease = state.window.lock().unwrap().as_ref().unwrap().1.clone();
    assert!(
        lease.validate().is_err(),
        "guarded owner must retire on 503"
    );
    assert_eq!(state.calls.load(Ordering::SeqCst), 1);

    state.held_responses.lock().unwrap().clear();
    state.window.lock().unwrap().take();
    let (status, _, _) = running.request("POST", "/write", &headers).await;
    assert_eq!(status, 200);
    assert_eq!(state.calls.load(Ordering::SeqCst), 2);

    assert!(running.stop().await);
    drop(state);
    drop(csrf);
    f.stop().await;
}
