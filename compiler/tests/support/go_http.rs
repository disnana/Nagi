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

fn health(socket: TcpStream, address: SocketAddr) {
    let head = format!("GET /health HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");
    let (response, body) = exchange(socket, &head, b"");
    assert!(response.starts_with("HTTP/1.1 200 "), "{response}");
    assert_eq!(body, b"ready");
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

fn verify(executable: &Path, stderr: &Path, body_limit: Option<usize>) {
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
    health(socket, address);
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
    health(TcpStream::connect(address).unwrap(), address);
}

#[test]
fn go_http_m21_fixed_length_binary_echo_and_body_limits_in_three_forms() {
    let fixture = Fixture::new();
    fixture.write("main.nagi", HIGH);
    fixture.write("manual/entry.low", LOW);
    success(fixture.command(&["lower", "main.nagi", "--no-project", "--out", "lowered"]));
    let stored = fixture.0.join("saved");
    fs::create_dir(&stored).unwrap();
    fs::copy(
        fixture.0.join("lowered/generated.low"),
        stored.join("entry.low"),
    )
    .unwrap();

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
