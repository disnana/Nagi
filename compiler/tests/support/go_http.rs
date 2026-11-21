use super::Fixture;
use std::{
    fs::{self, File},
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::Path,
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

const HIGH: &str = include_str!("../fixtures/go-http/echo.nagi");
const LOW: &str = include_str!("../fixtures/go-http/echo.low");

fn success(output: Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn exchange(mut socket: TcpStream, head: &str, body: &[u8]) -> (String, Vec<u8>) {
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    socket.write_all(head.as_bytes()).unwrap();
    socket.write_all(body).unwrap();
    let mut response = Vec::new();
    socket.read_to_end(&mut response).unwrap_or_else(|error| {
        panic!(
            "reading reply to {head:?}: {error}; received {} bytes",
            response.len()
        )
    });
    let split = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("HTTP response header separator");
    let body = response.split_off(split + 4);
    let header = String::from_utf8(response).unwrap();
    (header, body)
}

fn header<'a>(response: &'a str, name: &str) -> Option<&'a str> {
    response.lines().find_map(|line| {
        let (header, value) = line.split_once(':')?;
        header.eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

fn get(socket: TcpStream, address: SocketAddr, path: &str, expected: &[u8]) {
    let head = format!("GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");
    let (response, body) = exchange(socket, &head, b"");
    assert!(response.starts_with("HTTP/1.1 200 "), "{response}");
    assert_eq!(body, expected);
}

fn check_binary(socket: TcpStream, address: SocketAddr, payload: &[u8]) {
    let length = payload.len();
    let head = format!(
        "POST /echo HTTP/1.1\r\nHost: {address}\r\nContent-Length: {length}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n"
    );
    let (response, body) = exchange(socket, &head, payload);
    assert!(response.starts_with("HTTP/1.1 200 "), "{response}");
    assert_eq!(
        header(&response, "Content-Type"),
        Some("application/octet-stream")
    );
    assert_eq!(
        header(&response, "Content-Length").and_then(|value| value.parse().ok()),
        Some(length)
    );
    assert_eq!(body.len(), length);
    assert!(body == payload, "binary response differs from request");
}

fn start_server(
    executable: &Path,
    stderr: &Path,
    body_limit: Option<usize>,
) -> (Server, SocketAddr, TcpStream) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let mut command = Command::new(executable);
    command
        .env("NAGI_SAMPLE_PORT", address.port().to_string())
        .env("NAGI_HTTP_AUTHORITY", address.to_string())
        .env("NAGI_HTTP_BODY_LIMIT", body_limit.unwrap_or(0).to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(File::create(stderr).unwrap());
    drop(listener);
    let mut server = Server(command.spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(10);
    let socket = loop {
        match TcpStream::connect(address) {
            Ok(socket) => break socket,
            Err(problem) => {
                assert!(
                    server.0.try_wait().unwrap().is_none() && Instant::now() < deadline,
                    "server unavailable at {address}: {problem}\n{}",
                    fs::read_to_string(stderr).unwrap()
                );
                thread::sleep(Duration::from_millis(20));
            }
        }
    };
    (server, address, socket)
}

fn verify(executable: &Path, stderr: &Path, body_limit: Option<usize>) {
    let (_server, address, socket) = start_server(executable, stderr, body_limit);
    get(socket, address, "/health", b"ready");
    let limit = body_limit.unwrap_or(1024 * 1024);
    let octets = (0..=u8::MAX).collect::<Vec<_>>();
    check_binary(TcpStream::connect(address).unwrap(), address, &octets);
    let at_limit = octets.repeat(limit / octets.len());
    assert_eq!(at_limit.len(), limit);
    check_binary(TcpStream::connect(address).unwrap(), address, &at_limit);
    let over_limit = limit + 1;
    let head = format!(
        "POST /echo HTTP/1.1\r\nHost: {address}\r\nContent-Length: {over_limit}\r\nExpect: 100-continue\r\n\r\n"
    );
    let (response, _) = exchange(TcpStream::connect(address).unwrap(), &head, b"");
    assert!(response.starts_with("HTTP/1.1 413 "), "{response}");
    assert_eq!(header(&response, "Connection"), Some("close"));
    get(
        TcpStream::connect(address).unwrap(),
        address,
        "/health",
        b"ready",
    );
}

fn prepare(fixture: &Fixture, high: &str, low: &str) {
    fixture.write("main.nagi", high);
    fixture.write("manual/entry.low", low);
    success(fixture.command(&["lower", "main.nagi", "--no-project", "--out", "lowered"]));
    let stored = fixture.0.join("saved");
    fs::create_dir(&stored).unwrap();
    fs::copy(
        fixture.0.join("lowered/generated.low"),
        stored.join("entry.low"),
    )
    .unwrap();
}

fn verify_response_reservation(fixture: &Fixture) {
    let emitted = fixture.0.join("high/go");
    let probe = fixture.0.join("response-reservation");
    fs::create_dir(&probe).unwrap();
    for file in ["main.go", "go.mod"] {
        fs::copy(emitted.join(file), probe.join(file)).unwrap();
    }
    fs::write(
        probe.join("response_reservation_test.go"),
        include_str!("../fixtures/go-http/response_reservation_test.go"),
    )
    .unwrap();
    let go = std::env::var_os("NAGI_GO").unwrap_or_else(|| "go".into());
    success(
        Command::new(go)
            .current_dir(probe)
            .args([
                "test",
                "-count=1",
                "-run",
                "^TestNagiHTTPResponseReservation",
                ".",
            ])
            .env("GOTOOLCHAIN", "local")
            .env("GOWORK", "off")
            .output()
            .unwrap(),
    );
}

#[test]
fn go_http_m21_fixed_length_binary_echo_and_body_limits_in_three_forms() {
    let fixture = Fixture::new();
    prepare(&fixture, HIGH, LOW);

    for (source, out) in [
        ("main.nagi", "high"),
        ("saved/entry.low", "saved-low"),
        ("manual/entry.low", "manual-low"),
    ] {
        success(fixture.go("build", source, out));
        let (app, receipt) = fixture.latest(out);
        let executable = app.join(receipt["executable"].as_str().unwrap());
        let stderr = fixture.0.join(format!("{out}.stderr"));
        for body_limit in [None, Some(4096)] {
            verify(&executable, &stderr, body_limit);
            eprintln!("{out}: binary POST, bodyless GET and body limit {body_limit:?} passed");
        }
    }

    verify_response_reservation(&fixture);

    let use_after_move = format!("import std.ownership as ownership\n{HIGH}").replace(
        "    return http.bytes(http.response_context(view(request)), http.Status.OK, request.body)",
        "    moved = ownership.move(request)\n    return http.bytes(http.response_context(view(moved)), http.Status.OK, request.body)",
    );
    let awaiting_handler = HIGH.replace(
        "    assert_true(len(request.body) == 0)",
        "    await sleep(1)\n    assert_true(len(request.body) == 0)",
    );
    for (name, source, expected) in [
        ("moved.nagi", &use_after_move, "move後"),
        (
            "awaiting.nagi",
            &awaiting_handler,
            "Go HTTP handler must complete without await",
        ),
    ] {
        fixture.write(name, source);
        let denied = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(&fixture.0)
            .args([
                "build",
                name,
                "--backend",
                "go",
                "--no-project",
                "--out",
                "denied",
            ])
            .env("NAGI_GO", fixture.0.join("missing-go"))
            .output()
            .unwrap();
        let diagnostic = String::from_utf8_lossy(&denied.stderr);
        assert!(!denied.status.success(), "{diagnostic}");
        assert!(
            diagnostic.contains(expected) && diagnostic.contains(name),
            "{diagnostic}"
        );
        assert!(!fixture.0.join("denied/.nagi/apps").exists());
    }
}

#[test]
fn go_http_m39_direct_serve_shares_read_only_state_in_three_forms() {
    let fixture = Fixture::new();
    let high = include_str!("../fixtures/go-http/state.nagi");
    let low = include_str!("../fixtures/go-http/state.low");
    prepare(&fixture, high, low);

    for (source, out) in [
        ("main.nagi", "high"),
        ("saved/entry.low", "saved-low"),
        ("manual/entry.low", "manual-low"),
    ] {
        success(fixture.go("build", source, out));
        let (app, receipt) = fixture.latest(out);
        let executable = app.join(receipt["executable"].as_str().unwrap());
        let generated = fs::read_to_string(app.join(receipt["go"].as_str().unwrap())).unwrap();
        let stderr = fixture.0.join(format!("{out}.stderr"));
        let (_server, address, socket) = start_server(&executable, &stderr, None);
        get(socket, address, "/health", b"ready");
        get(
            TcpStream::connect(address).unwrap(),
            address,
            "/other",
            b"shared state",
        );
        get(
            TcpStream::connect(address).unwrap(),
            address,
            "/health",
            b"ready",
        );

        // The payload can give the same text even if it is copied per request.
        // Check the generated ABI and that dispatch passes the app's owner.
        assert!(
            generated.contains("state: &state"),
            "{source}: App must hold a shared state reference"
        );
        assert!(
            generated
                .lines()
                .any(|line| line.starts_with("func nagiFn_")
                    && line.contains("nagiHTTPRequest")
                    && line.contains("*nagiType_")),
            "{source}: generated handler must accept a state reference"
        );
        assert!(
            generated.contains("context: responseContext}, app.state)"),
            "{source}: dispatch must pass App's reference"
        );
        eprintln!("{out}: both immediate handlers and the shared State ABI passed");
    }

    let invalid = high.replace(
        "greeting = view(state.labels.ready)",
        "greeting = state.labels.ready",
    );
    let expected_line = invalid
        .lines()
        .position(|line| line.contains("greeting = state.labels.ready"))
        .unwrap()
        + 1;
    fixture.write("move-state.nagi", &invalid);
    let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .current_dir(&fixture.0)
        .args([
            "build",
            "move-state.nagi",
            "--no-project",
            "--backend",
            "go",
            "--out",
            "denied-state",
        ])
        .env("NAGI_GO", fixture.0.join("missing-go"))
        .output()
        .unwrap();
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{diagnostic}");
    assert!(
        diagnostic.contains("sharedまたは借用resourceの非Copy fieldはmoveできません")
            && diagnostic.contains(&format!("move-state.nagi:{expected_line}")),
        "{diagnostic}"
    );
    assert!(!fixture.0.join("denied-state/.nagi/apps").exists());
}

#[test]
fn go_http_cors_is_opt_in_through_the_cli_in_three_forms() {
    let fixture = Fixture::new();
    prepare(
        &fixture,
        include_str!("../fixtures/go-http/cors.nagi"),
        include_str!("../fixtures/go-http/cors.low"),
    );

    for (source, out) in [
        ("main.nagi", "high"),
        ("saved/entry.low", "saved-low"),
        ("manual/entry.low", "manual-low"),
    ] {
        success(fixture.go("build", source, out));
        let (app, receipt) = fixture.latest(out);
        let executable = app.join(receipt["executable"].as_str().unwrap());
        let stderr = fixture.0.join(format!("{out}.stderr"));
        let (_server, address, socket) = start_server(&executable, &stderr, None);
        get(socket, address, "/health", b"ready");

        let request = |method, path, headers| {
            let head = format!(
                "{method} {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n{headers}\r\n"
            );
            exchange(TcpStream::connect(address).unwrap(), &head, b"")
        };
        let origin = "Origin: https://APP.example.invalid:443\r\n";
        let (response, body) = request("GET", "/health", origin);
        assert!(response.starts_with("HTTP/1.1 200 "), "{response}");
        assert_eq!(body, b"ready");
        assert_eq!(
            header(&response, "Access-Control-Allow-Origin"),
            Some("https://app.example.invalid")
        );
        assert_eq!(header(&response, "Vary"), Some("Origin"));
        assert_eq!(header(&response, "Access-Control-Allow-Credentials"), None);

        let preflight = format!(
            "{origin}Access-Control-Request-Method: GET\r\nAccess-Control-Request-Headers: X-Trace, Content-Type\r\n"
        );
        let (response, body) = request("OPTIONS", "/health", preflight.as_str());
        assert!(response.starts_with("HTTP/1.1 204 "), "{response}");
        assert!(body.is_empty());
        for (name, value) in [
            ("Access-Control-Allow-Origin", "https://app.example.invalid"),
            ("Access-Control-Allow-Methods", "GET"),
            ("Access-Control-Allow-Headers", "content-type, x-trace"),
            ("Access-Control-Max-Age", "120"),
            ("Cache-Control", "no-store"),
        ] {
            assert_eq!(header(&response, name), Some(value), "{name}: {response}");
        }
        let vary = header(&response, "Vary").unwrap().to_ascii_lowercase();
        for name in [
            "origin",
            "access-control-request-method",
            "access-control-request-headers",
        ] {
            assert!(vary.split(',').any(|token| token.trim() == name), "{vary}");
        }

        let (response, body) = request("GET", "/other", origin);
        assert!(response.starts_with("HTTP/1.1 200 "), "{response}");
        assert_eq!(body, b"shared state");
        assert!(!response
            .to_ascii_lowercase()
            .contains("\r\naccess-control-"));
        assert_eq!(header(&response, "Vary"), None);
        let (response, _) = request("OPTIONS", "/other", preflight.as_str());
        assert!(response.starts_with("HTTP/1.1 403 "), "{response}");
        assert_eq!(header(&response, "Access-Control-Allow-Origin"), None);
        let (response, _) = request("OPTIONS", "/health", origin);
        assert!(response.starts_with("HTTP/1.1 405 "), "{response}");
        assert_eq!(header(&response, "Access-Control-Allow-Origin"), None);
        eprintln!("{out}: configured GET, preflight and default route behavior passed");
    }
}
