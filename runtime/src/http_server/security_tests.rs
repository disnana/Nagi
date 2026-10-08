use super::*;
use crate::auth::{AuthScope, Failure, FailureKind, Grant, VerifiedIdentity};
use std::sync::Mutex;
#[derive(Clone, Default)]
struct State {
    calls: Arc<AtomicUsize>,
    verifications: Arc<AtomicUsize>,
    verified: Arc<Notify>,
    proof: Arc<Mutex<Option<AuthScope>>>,
    grant: Arc<Mutex<Option<Grant<()>>>>,
}
async fn verifier(head: Request, state: Arc<State>) -> Result<VerifiedIdentity, Failure> {
    assert!(head.body().is_empty());
    state.verifications.fetch_add(1, Ordering::SeqCst);
    if header_text(&head, "authorization").unwrap() != Some("Bearer fixture") {
        return Err(Failure::invalid_credential());
    }
    VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(1))
}
async fn authenticated(_: Request, state: Arc<State>, proof: AuthScope) -> Result<Response, Error> {
    state.calls.fetch_add(1, Ordering::SeqCst);
    *state.proof.lock().unwrap() = Some(proof);
    Ok(text(Status::OK, "ready"))
}
fn credential_request(method: &str, credentials: &str) -> Vec<u8> {
    format!(
        "{method} /secure HTTP/1.1\r\nHost: localhost\r\n{credentials}Connection: close\r\n\r\n"
    )
    .into_bytes()
}
fn expired(state: &State) {
    let proof = state
        .proof
        .lock()
        .unwrap()
        .take()
        .expect("handler retained proof");
    assert_eq!(proof.validate().unwrap_err().kind(), FailureKind::Expired);
}
#[tokio::test]
async fn credentials_fail_closed_without_running_verifier_or_handler() {
    let state = State::default();
    let app = route(
        app_default(state.clone()),
        Method::GET,
        "/secure",
        authenticated_policy(verifier),
        authenticated,
    )
    .unwrap();
    let server = Server::new(app, default_options()).await;
    for (credentials, status) in [
        ("", 401),
        ("Authorization: Basic fixture\r\n", 401),
        ("Authorization: Bearer \r\n", 401),
        (
            "Authorization: Bearer fixture\r\nAuthorization: Bearer fixture\r\n",
            400,
        ),
        (
            "Authorization: Bearer fixture\r\nCookie: session=fixture\r\n",
            400,
        ),
    ] {
        let mut socket = server.connect().await;
        send(&mut socket, &credential_request("GET", credentials)).await;
        let received = response(&mut socket, false).await;
        assert_eq!(received.status, status);
        assert_eq!(received.all("x-content-type-options"), ["nosniff"]);
    }
    assert_eq!(state.calls.load(Ordering::SeqCst), 0);
    assert_eq!(state.verifications.load(Ordering::SeqCst), 0);
    let mut socket = server.connect().await;
    send(
        &mut socket,
        &credential_request("GET", "Authorization: Bearer rejected\r\n"),
    )
    .await;
    assert_eq!(response(&mut socket, false).await.status, 401);
    assert_eq!(state.verifications.load(Ordering::SeqCst), 1);
    assert_eq!(state.calls.load(Ordering::SeqCst), 0);
    server.stop().await;
}
#[tokio::test]
async fn head_inherits_get_policy_and_no_implicit_options_or_health() {
    let state = State::default();
    let app = route(
        app_default(state.clone()),
        Method::GET,
        "/secure",
        authenticated_policy(verifier),
        authenticated,
    )
    .unwrap();
    let server = Server::new(app, default_options()).await;
    let mut socket = server.connect().await;
    send(&mut socket, &credential_request("HEAD", "")).await;
    let received = response(&mut socket, true).await;
    assert_eq!(received.status, 401);
    assert!(received.body.is_empty());
    assert_eq!(state.calls.load(Ordering::SeqCst), 0);
    let mut socket = server.connect().await;
    send(
        &mut socket,
        &credential_request("HEAD", "Authorization: Bearer fixture\r\n"),
    )
    .await;
    let received = response(&mut socket, true).await;
    assert_eq!(received.status, 200);
    assert!(received.body.is_empty());
    expired(&state);
    let mut socket = server.connect().await;
    send(&mut socket, &credential_request("OPTIONS", "")).await;
    assert_eq!(response(&mut socket, false).await.status, 405);
    let mut socket = server.connect().await;
    send(
        &mut socket,
        b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    )
    .await;
    assert_eq!(response(&mut socket, false).await.status, 404);
    server.stop().await;
}
#[tokio::test]
async fn credential_expiry_during_body_receive_prevents_handler_admission() {
    async fn short_verifier(_: Request, state: Arc<State>) -> Result<VerifiedIdentity, Failure> {
        let identity =
            VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_millis(30))?;
        state.verified.notify_one();
        Ok(identity)
    }
    let state = State::default();
    let app = route(
        app_default(state.clone()),
        Method::POST,
        "/secure",
        authenticated_policy(short_verifier),
        authenticated,
    )
    .unwrap();
    let server = Server::new(app, default_options()).await;
    let mut socket = server.connect().await;
    send(&mut socket,b"POST /secure HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer fixture\r\nContent-Length: 1\r\nConnection: close\r\n\r\n").await;
    tokio::time::timeout(Duration::from_secs(1), state.verified.notified())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(80)).await;
    send(&mut socket, b"x").await;
    assert_eq!(response(&mut socket, false).await.status, 401);
    assert_eq!(state.calls.load(Ordering::SeqCst), 0);
    server.stop().await;
}
#[tokio::test]
async fn normal_err_panic_timeout_and_shutdown_invalidate_escaped_rust_scope() {
    for mode in 0..5 {
        let state = State::default();
        let observed = state.clone();
        let handler = move |_: Request, state: Arc<State>, proof: AuthScope| async move {
            state.calls.fetch_add(1, Ordering::SeqCst);
            *state.proof.lock().unwrap() = Some(proof);
            match mode {
                0 => Ok(text(Status::OK, "ready")),
                1 => Err(Error::internal("hidden fixture")),
                2 => panic!("hidden fixture"),
                _ => std::future::pending::<Result<Response, Error>>().await,
            }
        };
        let app = route(
            app_default(state),
            Method::GET,
            "/secure",
            authenticated_policy(verifier),
            handler,
        )
        .unwrap();
        let options = options(1024, 1000, if mode == 4 { 1000 } else { 20 }, 20).unwrap();
        let server = Server::new(app, options).await;
        let mut socket = server.connect().await;
        send(
            &mut socket,
            &credential_request("GET", "Authorization: Bearer fixture\r\n"),
        )
        .await;
        if mode == 4 {
            tokio::time::timeout(Duration::from_secs(1), async {
                while observed.calls.load(Ordering::SeqCst) == 0 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            server.stop().await;
        } else {
            let result = response(&mut socket, false).await;
            assert_eq!(result.status, [200, 500, 500, 504][mode]);
            assert!(!String::from_utf8_lossy(&result.body).contains("hidden fixture"));
            server.stop().await;
        }
        expired(&observed);
    }
}
#[tokio::test]
async fn authorizer_timeout_shares_budget_and_invalidates_intermediate_scope() {
    async fn pending_authorizer(
        scope: AuthScope,
        _: Request,
        state: Arc<State>,
    ) -> Result<Grant<()>, Failure> {
        *state.proof.lock().unwrap() = Some(scope);
        std::future::pending().await
    }
    let state = State::default();
    let app = route(
        app_default(state.clone()),
        Method::GET,
        "/secure",
        authorized_policy(verifier, pending_authorizer),
        |_: Request, _: Arc<State>, _: Grant<()>| async { Ok(text(Status::OK, "must not run")) },
    )
    .unwrap();
    let server = Server::new(app, security_timeout(default_options(), 20).unwrap()).await;
    let mut socket = server.connect().await;
    send(
        &mut socket,
        &credential_request("GET", "Authorization: Bearer fixture\r\n"),
    )
    .await;
    assert_eq!(response(&mut socket, false).await.status, 504);
    expired(&state);
    server.stop().await;
}
#[tokio::test]
async fn authorizer_cannot_return_a_grant_from_another_request() {
    async fn authorize(
        scope: AuthScope,
        _: Request,
        state: Arc<State>,
    ) -> Result<Grant<()>, Failure> {
        if let Some(old) = state.grant.lock().unwrap().take() {
            return Ok(old);
        }
        Grant::from_authorized(scope, 9)
    }
    async fn retain(_: Request, state: Arc<State>, grant: Grant<()>) -> Result<Response, Error> {
        state.calls.fetch_add(1, Ordering::SeqCst);
        *state.grant.lock().unwrap() = Some(grant);
        Ok(text(Status::OK, "ready"))
    }
    let state = State::default();
    let app = route(
        app_default(state.clone()),
        Method::GET,
        "/secure",
        authorized_policy(verifier, authorize),
        retain,
    )
    .unwrap();
    let server = Server::new(app, default_options()).await;
    for status in [200, 400] {
        let mut socket = server.connect().await;
        send(
            &mut socket,
            &credential_request("GET", "Authorization: Bearer fixture\r\n"),
        )
        .await;
        assert_eq!(response(&mut socket, false).await.status, status);
    }
    assert_eq!(state.calls.load(Ordering::SeqCst), 1);
    server.stop().await;
}
#[tokio::test]
async fn late_verifier_failure_uses_the_exhausted_security_budget() {
    async fn late(_: Request, _: Arc<State>) -> Result<VerifiedIdentity, Failure> {
        std::thread::sleep(Duration::from_millis(20));
        Err(Failure::denied())
    }
    let state = State::default();
    let app = route(
        app_default(state.clone()),
        Method::GET,
        "/secure",
        authenticated_policy(late),
        authenticated,
    )
    .unwrap();
    let server = Server::new(app, security_timeout(default_options(), 5).unwrap()).await;
    let mut socket = server.connect().await;
    send(
        &mut socket,
        &credential_request("GET", "Authorization: Bearer fixture\r\n"),
    )
    .await;
    assert_eq!(response(&mut socket, false).await.status, 504);
    assert_eq!(state.calls.load(Ordering::SeqCst), 0);
    server.stop().await;
}
#[tokio::test]
async fn all_security_failure_kinds_have_stable_wire_status() {
    for (kind, status, message) in [
        (FailureKind::InvalidCredential, 401, "invalid credential"),
        (FailureKind::Denied, 403, "permission denied"),
        (FailureKind::Expired, 401, "authority expired"),
        (FailureKind::InvalidRequest, 400, "invalid security request"),
        (
            FailureKind::Unavailable,
            503,
            "security service unavailable",
        ),
        (FailureKind::Internal, 500, "security service failure"),
    ] {
        let state = State::default();
        let verify = move |_: Request, _: Arc<State>| async move {
            Err::<VerifiedIdentity, Failure>(match kind {
                FailureKind::InvalidCredential => Failure::invalid_credential(),
                FailureKind::Denied => Failure::denied(),
                FailureKind::Expired => Failure::expired(),
                FailureKind::InvalidRequest => Failure::invalid_request(),
                FailureKind::Unavailable => Failure::unavailable(),
                FailureKind::Internal => Failure::internal(),
            })
        };
        let app = route(
            app_default(state.clone()),
            Method::GET,
            "/secure",
            authenticated_policy(verify),
            authenticated,
        )
        .unwrap();
        let server = Server::new(app, default_options()).await;
        let mut socket = server.connect().await;
        send(
            &mut socket,
            &credential_request("GET", "Authorization: Bearer fixture\r\n"),
        )
        .await;
        let received = response(&mut socket, false).await;
        assert_eq!(received.status, status);
        assert_eq!(received.body, message.as_bytes());
        assert_eq!(received.all("x-content-type-options"), ["nosniff"]);
        if status == 401 {
            assert_eq!(received.all("www-authenticate"), ["Bearer"]);
        }
        assert_eq!(state.calls.load(Ordering::SeqCst), 0);
        server.stop().await;
    }
}
#[tokio::test]
async fn verifier_and_authorizer_constructor_and_poll_panics_fail_closed() {
    for mode in 0..4 {
        let state = State::default();
        let verify = move |request: Request, state: Arc<State>| {
            if mode == 0 {
                panic!("secret verifier constructor");
            }
            async move {
                if mode == 1 {
                    panic!("secret verifier poll");
                }
                verifier(request, state).await
            }
        };
        let authorize = move |scope: AuthScope, _: Request, state: Arc<State>| {
            *state.proof.lock().unwrap() = Some(scope);
            if mode == 2 {
                panic!("secret authorizer constructor");
            }
            async move {
                panic!("secret authorizer poll");
                #[allow(unreachable_code)]
                Err::<Grant<()>, Failure>(Failure::internal())
            }
        };
        let app = route(
            app_default(state.clone()),
            Method::GET,
            "/secure",
            authorized_policy(verify, authorize),
            |_: Request, _: Arc<State>, _: Grant<()>| async { Ok(text(Status::OK, "unreachable")) },
        )
        .unwrap();
        let server = Server::new(app, capacity(default_options(), 2, 1).unwrap()).await;
        for _ in 0..2 {
            let mut socket = server.connect().await;
            send(
                &mut socket,
                &credential_request("GET", "Authorization: Bearer fixture\r\n"),
            )
            .await;
            let received = response(&mut socket, false).await;
            assert_eq!(received.status, 500);
            assert!(!String::from_utf8_lossy(&received.body).contains("secret"));
            if mode >= 2 {
                expired(&state);
            }
        }
        server.stop().await;
    }
}
#[tokio::test]
async fn dropping_unpolled_prepared_handler_cannot_reactivate_proof() {
    let state = State::default();
    let app = route(
        app_default(state.clone()),
        Method::GET,
        "/secure",
        authenticated_policy(verifier),
        authenticated,
    )
    .unwrap();
    let owner = LeaseOwner::new(Instant::now() + Duration::from_secs(1)).unwrap();
    let lease = Arc::clone(&owner.lease);
    let mut headers = HeaderMap::new();
    headers.insert(
        names::AUTHORIZATION,
        HeaderValue::from_static("Bearer fixture"),
    );
    let request = Request {
        method: Method::GET,
        uri: "/secure".parse().unwrap(),
        headers,
        body: Bytes::new(),
    };
    let prepared =
        (app.routes["/secure"][0].prepare)(request, Arc::clone(&app.state), Arc::clone(&lease))
            .await
            .unwrap();
    let request = Request {
        method: Method::GET,
        uri: "/secure".parse().unwrap(),
        headers: HeaderMap::new(),
        body: Bytes::new(),
    };
    let future = prepared(request, Arc::clone(&app.state));
    drop(owner);
    drop(future);
    assert_eq!(state.calls.load(Ordering::SeqCst), 0);
    assert_eq!(lease.validate().unwrap_err().kind(), FailureKind::Expired);
}
#[tokio::test]
async fn mapper_panic_invalidates_the_same_request_proof() {
    async fn failure(_: Request, state: Arc<State>, scope: AuthScope) -> Result<Response, Error> {
        *state.proof.lock().unwrap() = Some(scope);
        Err(Error::internal("private"))
    }
    fn panic_mapper(_: Error) -> Response {
        panic!("private mapper");
    }
    let state = State::default();
    let app = route(
        app(state.clone(), panic_mapper),
        Method::GET,
        "/secure",
        authenticated_policy(verifier),
        failure,
    )
    .unwrap();
    let server = Server::new(app, default_options()).await;
    let mut socket = server.connect().await;
    send(
        &mut socket,
        &credential_request("GET", "Authorization: Bearer fixture\r\n"),
    )
    .await;
    let received = response(&mut socket, false).await;
    assert_eq!(received.status, 500);
    assert!(!String::from_utf8_lossy(&received.body).contains("private"));
    expired(&state);
    server.stop().await;
}
