use super::*;

fn checked_hosts(hosts: &[&str]) -> Options {
    authority(
        default_options(),
        "https://localhost",
        hosts.iter().map(|s| (*s).to_owned()).collect(),
        8,
        512,
    )
    .unwrap()
}

#[test]
fn authority_startup_checks_ports_origin_duplicates_and_proxy_profile() {
    for host in [
        "",
        "localhost:",
        "localhost:0",
        "localhost:01",
        "localhost:+1",
        "localhost:65536",
        "localhost:no",
        "user@localhost",
        "localhost.",
        "localhost,other",
        "[::1%lo0]",
        "::1",
        "127.00.0.1",
        "999.1.1.1",
        "é.test",
    ] {
        assert!(authority(
            default_options(),
            "https://localhost",
            vec![host.to_owned()],
            2,
            512
        )
        .is_err());
    }
    for origin in [
        "http://localhost",
        "https://localhost/",
        "https://user@localhost",
        "https://localhost?x",
        "https://localhost#x",
    ] {
        assert!(authority(
            default_options(),
            origin,
            vec!["localhost".to_owned()],
            2,
            512
        )
        .is_err());
    }
    for hosts in [
        vec!["LOCALHOST", "localhost"],
        vec!["[::1]", "[0:0:0:0:0:0:0:1]"],
    ] {
        assert!(authority(
            default_options(),
            "https://localhost",
            hosts.into_iter().map(str::to_owned).collect(),
            2,
            512
        )
        .is_err());
    }
    for (count, bytes) in [(0, 512), (2, 0), (1, 8)] {
        assert!(authority(
            default_options(),
            "https://localhost",
            vec!["localhost".to_owned()],
            count,
            bytes
        )
        .is_err());
    }
    assert!(authority(default_options(), "https://localhost", Vec::new(), 2, 512).is_err());
    assert!(authority(
        default_options(),
        "https://localhost",
        vec!["localhost".to_owned(), "other".to_owned()],
        1,
        512
    )
    .is_err());
    assert!(authority(
        default_options(),
        "https://localhost",
        vec!["localhost".to_owned(), "other-long-host".to_owned()],
        2,
        20
    )
    .is_err());
    assert!(trusted_proxy(default_options(), vec!["127.0.0.1".to_owned()]).is_err());
    for peers in [
        Vec::new(),
        vec!["bad"],
        vec!["0.0.0.0"],
        vec!["::"],
        vec!["127.0.0.1", "127.0.0.1"],
        vec!["127.0.0.1", "::ffff:127.0.0.1"],
    ] {
        assert!(trusted_proxy(
            checked_hosts(&["localhost"]),
            peers.into_iter().map(str::to_owned).collect()
        )
        .is_err());
    }
    checked_hosts(&["localhost", "localhost:443", "[::1]", "127.0.0.1:65535"]);
    assert!(authority(
        checked_hosts(&["localhost"]),
        "https://other",
        vec!["other".to_owned()],
        2,
        512
    )
    .is_err());
    let proxy = trusted_proxy(checked_hosts(&["localhost"]), vec!["127.0.0.1".to_owned()]).unwrap();
    assert!(trusted_proxy(proxy, vec!["203.0.113.1".to_owned()]).is_err());
}

#[tokio::test]
async fn authority_rejects_before_unknown_route_and_recovers_capacity() {
    let calls = Arc::new(AtomicUsize::new(0));
    let app = route(
        app_default(calls.clone()),
        Method::GET,
        "/",
        public_policy(),
        |_, calls, ()| async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(empty(Status::OK))
        },
    )
    .unwrap();
    let options = capacity(
        checked_hosts(&["localhost", "localhost:443", "[::1]"]),
        2,
        1,
    )
    .unwrap();
    let server = Server::new(app, options).await;
    for (target, version, headers, expected) in [
        ("/missing", "HTTP/1.1", "Host: wrong", 400),
        ("/", "HTTP/1.1", "Host: localhost\r\nHost: localhost", 400),
        ("/", "HTTP/1.1", "", 400),
        ("/", "HTTP/1.1", "Host: localhost:0443", 400),
        ("/", "HTTP/1.1", "Host: localhost:no", 400),
        ("/", "HTTP/1.1", "Host: localhost:80", 400),
        ("/", "HTTP/1.1", "Host: user@localhost", 400),
        (
            "/",
            "HTTP/1.1",
            "Host: localhost\r\nForwarded: host=localhost",
            400,
        ),
        (
            "/",
            "HTTP/1.1",
            "Host: localhost\r\nX-Forwarded-For: 127.0.0.1",
            400,
        ),
        ("https://localhost/", "HTTP/1.1", "Host: localhost", 400),
        ("*", "HTTP/1.1", "Host: localhost", 400),
        ("/", "HTTP/1.0", "Host: localhost", 505),
    ] {
        let before = calls.load(Ordering::SeqCst);
        let mut socket = server.connect().await;
        send(
            &mut socket,
            format!("GET {target} {version}\r\n{headers}\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .await;
        let rejected = response(&mut socket, false).await;
        assert_eq!(rejected.status, expected);
        assert_eq!(rejected.all("cache-control"), ["no-store"]);
        assert_eq!(calls.load(Ordering::SeqCst), before);
        assert!(closed(&mut socket).await.is_empty());
        let mut valid = server.connect().await;
        send(
            &mut valid,
            b"GET / HTTP/1.1\r\nHost: LOCALHOST\r\nConnection: close\r\n\r\n",
        )
        .await;
        assert_eq!(response(&mut valid, false).await.status, 200);
        assert!(closed(&mut valid).await.is_empty());
    }
    for host in ["localhost:443", "[0:0:0:0:0:0:0:1]"] {
        let mut socket = server.connect().await;
        send(
            &mut socket,
            format!("GET / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .await;
        assert_eq!(response(&mut socket, false).await.status, 200);
        assert!(closed(&mut socket).await.is_empty());
    }
    server.stop().await;
}

#[tokio::test]
async fn proxy_acl_uses_actual_tcp_peer_and_rejects_forwarded_metadata() {
    for (peer, expected) in [
        ("203.0.113.1", 400),
        ("127.0.0.1", 200),
        ("::ffff:127.0.0.1", 200),
    ] {
        let calls = Arc::new(AtomicUsize::new(0));
        let app = route(
            app_default(calls.clone()),
            Method::GET,
            "/",
            public_policy(),
            |_, calls, ()| async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(empty(Status::OK))
            },
        )
        .unwrap();
        let options = trusted_proxy(checked_hosts(&["localhost"]), vec![peer.to_owned()]).unwrap();
        let server = Server::new(app, options).await;
        let mut socket = server.connect().await;
        send(
            &mut socket,
            b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .await;
        assert_eq!(response(&mut socket, false).await.status, expected);
        assert_eq!(calls.load(Ordering::SeqCst), usize::from(expected == 200));
        assert!(closed(&mut socket).await.is_empty());
        let mut socket = server.connect().await;
        send(&mut socket, b"GET / HTTP/1.1\r\nHost: localhost\r\nX-Forwarded-Host: localhost\r\nConnection: close\r\n\r\n").await;
        assert_eq!(response(&mut socket, false).await.status, 400);
        let mut socket = server.connect().await;
        send(
            &mut socket,
            b"GET / HTTP/1.1\r\nHost: localhost:443\r\nConnection: close\r\n\r\n",
        )
        .await;
        assert_eq!(response(&mut socket, false).await.status, 400);
        server.stop().await;
    }
}

#[tokio::test]
async fn invalid_authority_never_invokes_bearer_verifier_or_handler() {
    struct Calls {
        verified: AtomicUsize,
        handled: AtomicUsize,
    }
    let app = app_default(Calls {
        verified: AtomicUsize::new(0),
        handled: AtomicUsize::new(0),
    });
    let calls = app.state.clone();
    let policy = authenticated_policy(|_, state: Arc<Calls>| async move {
        state.verified.fetch_add(1, Ordering::SeqCst);
        VerifiedIdentity::from_verified(1, Instant::now() + Duration::from_secs(2))
    });
    let app = route(
        app,
        Method::GET,
        "/",
        policy,
        |_, state, _scope: AuthScope| async move {
            state.handled.fetch_add(1, Ordering::SeqCst);
            Ok(empty(Status::OK))
        },
    )
    .unwrap();
    let server = Server::new(app, checked_hosts(&["localhost"])).await;
    for (method, path) in [
        ("GET", "/"),
        ("POST", "/"),
        ("OPTIONS", "/missing"),
        ("HEAD", "/"),
    ] {
        let mut socket = server.connect().await;
        send(
            &mut socket,
            format!("{method} {path} HTTP/1.1\r\nHost: rejected\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await;
        assert_eq!(response(&mut socket, method == "HEAD").await.status, 400);
        assert_eq!(calls.verified.load(Ordering::SeqCst), 0);
        assert_eq!(calls.handled.load(Ordering::SeqCst), 0);
        assert!(closed(&mut socket).await.is_empty());
    }
    server.stop().await;
}

#[tokio::test]
async fn unconfigured_authority_fails_startup_without_polling_shutdown() {
    let polled = Arc::new(AtomicUsize::new(0));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let observed = polled.clone();
    let result = serve_listener(listener, app_default(()), default_options(), async move {
        observed.fetch_add(1, Ordering::SeqCst);
    })
    .await;
    assert!(matches!(result.unwrap_err().kind, ErrorKind::Invalid));
    assert_eq!(polled.load(Ordering::SeqCst), 0);
    assert!(serve(app_default(()), 8080, default_options())
        .await
        .is_err());
}
