//! Registration/startup contract only; no credential or request attack fixture.
use super::*;

fn session_source() -> Policy<(), ()> {
    Policy {
        source: PolicySource::Session,
        factory: Arc::new(|_, _, _| panic!("registration must not invoke policy")),
    }
}

async fn handler(_: Request, _: Arc<()>, _: ()) -> Result<Response, Error> {
    panic!("registration/startup must not invoke handler")
}

fn assert_csrf_refusal<T: std::fmt::Debug>(result: Result<T, Error>) {
    let error = result.expect_err("unguarded unsafe Session route must be refused");
    assert!(matches!(error.kind, ErrorKind::Invalid));
    assert!(error.message.contains("CSRF"));
}

#[test]
fn unsafe_session_registration_requires_guard_before_policy_or_handler() {
    for method in [
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::DELETE,
        Method::TRACE,
        Method::CONNECT,
        Method::from_bytes(b"PROPFIND").unwrap(),
    ] {
        assert_csrf_refusal(route(
            app_default(()),
            method.clone(),
            "/owned",
            session_source(),
            handler,
        ));
        assert_csrf_refusal(route_mapped(
            app_default(()),
            method,
            "/owned",
            session_source(),
            handler,
            default_error,
        ));
    }
    for method in [Method::GET, Method::HEAD, Method::OPTIONS] {
        assert!(route(app_default(()), method, "/owned", session_source(), handler).is_ok());
    }
    // An explicit non-cookie source keeps its existing registration behavior.
    for source in [PolicySource::Public, PolicySource::Bearer] {
        let mut policy = session_source();
        policy.source = source;
        assert!(route(app_default(()), Method::POST, "/owned", policy, handler).is_ok());
    }
}

fn invalid_internal_app() -> App<(), Error> {
    // Private impossible-state fixture checks the startup boundary independently
    // of route registration. No production factory or bypass is introduced.
    let mut app = route(
        app_default(()),
        Method::POST,
        "/owned",
        public_policy(),
        handler,
    )
    .unwrap();
    app.routes.get_mut("/owned").unwrap()[0].source = PolicySource::Session;
    app
}

fn checked_options() -> Options {
    authority(
        default_options(),
        "https://localhost",
        vec!["localhost".to_owned()],
        1,
        128,
    )
    .unwrap()
}

#[tokio::test]
async fn unsafe_session_serve_refuses_before_binding() {
    // An occupied valid port would produce a bind error if startup forgot
    // this policy validation. Guard refusal must happen before that attempt.
    let occupied = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = i64::from(occupied.local_addr().unwrap().port());
    assert_csrf_refusal(serve(invalid_internal_app(), port, checked_options()).await);
}

#[tokio::test]
async fn unsafe_session_listener_refuses_before_accept_or_shutdown_poll() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut queued = tokio::net::TcpStream::connect(address).await.unwrap();
    let result = serve_listener(listener, invalid_internal_app(), checked_options(), async {
        panic!("invalid startup must not poll shutdown")
    })
    .await;
    assert_csrf_refusal(result);
    use tokio::io::AsyncReadExt;
    let mut byte = [0];
    let closed = tokio::time::timeout(Duration::from_secs(2), queued.read(&mut byte))
        .await
        .unwrap();
    assert!(closed.is_err() || closed.is_ok_and(|n| n == 0));
    drop(queued);
    // The owned listener was dropped on rejection, so the port is reclaimable.
    drop(TcpListener::bind(address).await.unwrap());
}
