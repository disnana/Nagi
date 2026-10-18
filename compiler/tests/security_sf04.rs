//! Small typed-HTML business applications and exact checker-stage refusals.
//! Native Cargo remains the standard integration path; direct slices are recorded separately.
#[path = "support/native_triple.rs"]
mod native_triple;
use nagic::{check, emit, source};
use std::fs;

const NATIVE_HTTP_ADAPTER: &str = r#"
mod native_http_adapter {
    use nagi_runtime::{http_server as http, Error};

    pub fn get(response: fn(http::Request) -> Result<http::Response, Error>) -> (u16, Vec<u8>) {
        ::tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let app = http::route(
                    http::app_default(()),
                    http::Method::GET,
                    "/",
                    http::public_policy(),
                    move |request, _state, _authority| {
                        let response = response;
                        async move { response(request) }
                    },
                )
                .unwrap();
                let app = http::rate_route(
                    app,
                    http::Method::GET,
                    "/",
                    http::rate_policy(1_000_000, 1000).unwrap(),
                )
                .unwrap();
                let listener = ::tokio::net::TcpListener::bind("127.0.0.1:0")
                    .await
                    .unwrap();
                let address = listener.local_addr().unwrap();
                let response_limits = http::response_limits(
                    1_048_576,
                    8192,
                    32,
                    8192,
                    32,
                    65_536,
                    256,
                    83_886_080,
                    1024,
                    285_212_672,
                    8192,
                    32,
                )
                .unwrap();
                let options =
                    http::with_response_limits(http::default_options(), response_limits).unwrap();
                let rate_limits = http::rate_limits(64, 64, 4096, 4_194_304, 65_536).unwrap();
                let options = http::with_rate_limits(options, rate_limits).unwrap();
                let options = http::authority(
                    options,
                    "https://localhost",
                    vec!["localhost".to_owned()],
                    1,
                    128,
                )
                .unwrap();
                let (stop, shutdown) = ::tokio::sync::oneshot::channel();
                let task = ::tokio::spawn(http::serve_listener(listener, app, options, async {
                    let _ = shutdown.await;
                }));
                let received = ::tokio::task::spawn_blocking(move || {
                    use ::std::io::{Read, Write};
                    let mut stream = ::std::net::TcpStream::connect_timeout(
                        &address,
                        ::std::time::Duration::from_secs(3),
                    )
                    .unwrap();
                    stream
                        .set_read_timeout(Some(::std::time::Duration::from_secs(3)))
                        .unwrap();
                    stream
                        .set_write_timeout(Some(::std::time::Duration::from_secs(3)))
                        .unwrap();
                    stream
                        .write_all(
                            b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
                        )
                        .unwrap();
                    let mut raw = Vec::new();
                    stream.take(65537).read_to_end(&mut raw).unwrap();
                    assert!(raw.len() <= 65536, "bounded native HTTP fixture response");
                    let separator = raw
                        .windows(4)
                        .position(|window| window == b"\r\n\r\n")
                        .expect("HTTP response headers");
                    let head = ::std::str::from_utf8(&raw[..separator]).unwrap();
                    let status = head
                        .lines()
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .nth(1)
                        .unwrap()
                        .parse()
                        .unwrap();
                    (status, raw[separator + 4..].to_vec())
                })
                .await
                .unwrap();
                stop.send(()).unwrap();
                ::tokio::time::timeout(::std::time::Duration::from_secs(3), task)
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();
                received
            })
    }
}
"#;

#[test]
fn list_detail_body_is_exact_in_three_source_forms() {
    native_triple::Fixture::new().run_three(
        "sf04-list-detail",
        include_str!("fixtures/security-sf04/list-detail.nagi"),
        include_str!("fixtures/security-sf04/list-detail-manual.low"),
        NATIVE_HTTP_ADAPTER,
        &format!(
            "#[test] fn native_body() {{ let (status, body) = native_http_adapter::get(build_page); assert_eq!(status, 200); assert_eq!(body, {:?}.as_bytes()); }}",
            include_str!("fixtures/security-sf04/expected-list-detail.html")
        ),
    );
}

#[test]
fn renamed_imports_build_the_page_and_keep_user_type_in_three_source_forms() {
    native_triple::Fixture::new().run_three(
        "sf04-renamed",
        include_str!("fixtures/security-sf04/renamed-page.nagi"),
        include_str!("fixtures/security-sf04/renamed-page-manual.low"),
        NATIVE_HTTP_ADAPTER,
        &format!(
            "#[test] fn native_body() {{ let (status, body) = native_http_adapter::get(build_page); assert_eq!(status, 201); assert_eq!(body, {:?}.as_bytes()); assert_eq!(user_value(), 7); }}",
            include_str!("fixtures/security-sf04/expected-renamed.html")
        ),
    );
}

#[test]
fn http_only_plain_response_keeps_user_type_in_three_source_forms() {
    native_triple::Fixture::new().run_three(
        "sf04-http-only",
        include_str!("fixtures/security-sf04/http-only.nagi"),
        include_str!("fixtures/security-sf04/http-only-manual.low"),
        NATIVE_HTTP_ADAPTER,
        "#[test] fn native_body() { let (status, body) = native_http_adapter::get(plain); assert_eq!(status, 200); assert_eq!(body, b\"plain\"); assert_eq!(user_value(), 9); }",
    );
}

fn reject_checked(
    fixture: &native_triple::Fixture,
    name: &str,
    text: &str,
    reason: &str,
    marker: &str,
) {
    let lines: Vec<_> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(marker))
        .map(|(line, _)| line + 1)
        .collect();
    assert_eq!(lines.len(), 1, "one offending marker in {name}");
    fixture.write(name, text);
    let mut loaded = source::load(&fixture.0.join(name), name.ends_with(".nagi"))
        .unwrap_or_else(|error| panic!("not a semantic refusal: {name}: {error}"));
    let error = check::check(&mut loaded.program).expect_err("checker accepted a negative");
    let diagnostic = loaded.diagnostic(&error);
    assert!(diagnostic.contains(reason), "{name}: {diagnostic}");
    assert!(
        diagnostic.contains(&format!("{name}:{}\n", lines[0])),
        "wrong primary input line: {diagnostic}"
    );
}

#[test]
fn ownership_opacity_and_nominal_arguments_reject_at_checker_in_three_forms() {
    let fixture = native_triple::Fixture::new();
    for (case, high, manual, reason, marker) in [
        (
            "builtin-copy",
            include_str!("fixtures/security-sf04/builtin-copy.nagi"),
            include_str!("fixtures/security-sf04/builtin-copy-manual.low"),
            "Clone",
            "copy(view(fragment))",
        ),
        (
            "opaque-field",
            include_str!("fixtures/security-sf04/opaque-field.nagi"),
            include_str!("fixtures/security-sf04/opaque-field-manual.low"),
            "encoded",
            "fragment.encoded",
        ),
        (
            "use-after-consume",
            include_str!("fixtures/security-sf04/use-after-consume.nagi"),
            include_str!("fixtures/security-sf04/use-after-consume-manual.low"),
            "move",
            "return",
        ),
        (
            "borrowed-document",
            include_str!("fixtures/security-sf04/borrowed-document.nagi"),
            include_str!("fixtures/security-sf04/borrowed-document-manual.low"),
            "型",
            "return",
        ),
        (
            "nominal-document",
            include_str!("fixtures/security-sf04/nominal-document.nagi"),
            include_str!("fixtures/security-sf04/nominal-document-manual.low"),
            "型",
            "return",
        ),
        (
            "shared-document",
            include_str!("fixtures/security-sf04/shared-document.nagi"),
            include_str!("fixtures/security-sf04/shared-document-manual.low"),
            "型",
            "return",
        ),
    ] {
        // Only the existing parsed-program Low printer prepares rejected saved Low;
        // it is not claimed as successful checked CLI lowering of invalid High.
        let high_name = format!("{case}.nagi");
        fixture.write(&high_name, high);
        let parsed =
            source::load(&fixture.0.join(&high_name), true).expect("negative must parse/load");
        let saved = emit::low(&parsed.program);
        let high_path = fixture.0.join(&high_name);
        fs::remove_file(&high_path).unwrap();
        for (name, text) in [
            (high_name, high),
            (format!("{case}-saved.low"), saved.as_str()),
            (format!("{case}-manual.low"), manual),
        ] {
            if name.ends_with(".low") {
                assert!(!high_path.exists(), "saved/manual Low must be independent");
            }
            reject_checked(&fixture, &name, text, reason, marker);
            if name.ends_with(".nagi") {
                fs::remove_file(&high_path).unwrap();
            }
        }
    }
}
