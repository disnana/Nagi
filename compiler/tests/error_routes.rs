use nagic::{check, emit, source};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static SOURCE_ID: AtomicU64 = AtomicU64::new(0);

fn load_program(text: &str, high: bool) -> nagic::ast::Program {
    let extension = if high { "nagi" } else { "low" };
    let path = std::env::temp_dir().join(format!(
        "nagi-http-json-check-{}-{}.{}",
        std::process::id(),
        SOURCE_ID.fetch_add(1, Ordering::Relaxed),
        extension,
    ));
    fs::write(&path, text).unwrap();
    let program = source::load(&path, high).unwrap().program;
    let _ = fs::remove_file(path);
    program
}

fn rejected_in_high_and_saved_low(source: &str, expected: &str) {
    let high = load_program(source, true);
    let low = emit::low(&high);
    for (text, is_high) in [(source, true), (low.as_str(), false)] {
        let mut program = load_program(text, is_high);
        let error = check::check(&mut program).expect_err("invalid HTTP JSON boundary accepted");
        assert!(error.contains(expected), "{error}\n{text}");
    }
}

#[test]
fn private_error_data_is_rejected_at_explicit_json_boundaries_in_high_and_low() {
    let declarations = [
        "class Private:\n    cause: Error\n",
        "enum Failure:\n    Missing\nclass Private:\n    cause: Failure\n",
    ];

    for declaration in declarations {
        // The route receives raw Request bytes. Application code explicitly
        // decodes them, so private fields retain the Deserialize diagnostic.
        let input = format!(
            "import std.http.server as http\n{declaration}class State:\n    count: i64\nasync def handler(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:\n    decoded = try json_decode[Private](request.body)\n    return http.empty(http.response_context(view(request)), http.Status.OK)\ndef setup() -> Result[http.App[State, Error], Error]:\n    app = http.app_default[State](State(count=0))\n    return http.route(app, http.Method.POST, view(\"/input\"), http.public_policy[State](), handler)\n"
        );
        rejected_in_high_and_saved_low(
            &input,
            "はJSONの読み取りに対応していません（Deserializeが必要です）",
        );

        for (parameter, json_type) in [
            ("value: Private", "Private"),
            ("values: List[Private]", "List[Private]"),
            ("value: Private?", "Private?"),
        ] {
            let output = format!(
                "import std.http.server as http\n{declaration}class State:\n    count: i64\nasync def handler(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:\n    return http.empty(http.response_context(view(request)), http.Status.OK)\ndef setup() -> Result[http.App[State, Error], Error]:\n    app = http.app_default[State](State(count=0))\n    app = try http.route(app, http.Method.GET, view(\"/output\"), http.public_policy[State](), handler)\n    return ok(app)\ndef encode(context: view[http.ResponseContext], {parameter}) -> Result[http.Response, Error]:\n    return http.json[{json_type}](context, http.Status.OK, {})\n",
                if parameter.starts_with("values:") { "values" } else { "value" },
            );
            rejected_in_high_and_saved_low(
                &output,
                "HTTP JSON builderの値はSerializeに対応していません",
            );
        }
    }

    let direct_enum = "import std.http.server as http\nenum Failure:\n    Missing\nclass State:\n    count: i64\nasync def handler(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:\n    return http.empty(http.response_context(view(request)), http.Status.OK)\ndef setup() -> Result[http.App[State, Error], Error]:\n    app = http.app_default[State](State(count=0))\n    app = try http.route(app, http.Method.GET, view(\"/output\"), http.public_policy[State](), handler)\n    return ok(app)\ndef encode(context: view[http.ResponseContext], value: Failure) -> Result[http.Response, Error]:\n    return http.json[Failure](context, http.Status.OK, value)\n";
    rejected_in_high_and_saved_low(
        direct_enum,
        "HTTP JSON builderの値はSerializeに対応していません",
    );
}

#[test]
fn explicit_http_json_routes_preserve_data_and_optional_payloads_in_high_and_low() {
    let source = r#"import std.http.server as http
class Input:
    id: i64
class Output:
    id: i64
class State:
    count: i64
async def data(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    context = http.response_context(view(request))
    input = try json_decode[Input](request.body)
    response = try http.json[Output](context, http.Status.OK, Output(id=input.id))
    roundtrip = try json_decode[Output](response.body)
    assert_true(roundtrip.id == 7)
    return ok(response)
async def optional(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    context = http.response_context(view(request))
    response = try http.json[Output?](context, http.Status.OK, None)
    assert_true(len(response.body) == 4)
    decoded_absent = try json_decode[Output?](response.body)
    match decoded_absent:
        case None:
            return ok(response)
        case Some(_):
            return error("optional null changed representation")
def setup() -> Result[http.App[State, Error], Error]:
    app = http.app_default[State](State(count=0))
    app = try http.route(app, http.Method.POST, view("/data"), http.public_policy[State](), data)
    policy = try http.rate_policy(10, 1000)
    app = try http.rate_route(app, http.Method.POST, "/data", policy)
    app = try http.route(app, http.Method.GET, view("/optional"), http.public_policy[State](), optional)
    policy = try http.rate_policy(10, 1000)
    return http.rate_route(app, http.Method.GET, "/optional", policy)
@rust("native::exercise")
extern async def exercise(app: http.App[State, Error]) -> Result[unit, Error]
async def main() -> Result[unit, Error]:
    app = try setup()
    try await exercise(app)
    return ok(print("HTTP JSON roundtrip checked"))
"#;
    let mut high = load_program(source, true);
    check::check(&mut high).unwrap();
    let low = emit::low(&high);
    let fixture = Fixture::new();
    fs::write(
        fixture.path.join("nagi.toml"),
        "entry = \"main.nagi\"\n[rust.dependencies]\ntokio = { version = \"1.48\", features = [\"net\", \"io-util\", \"sync\", \"time\", \"rt\"] }\n",
    )
    .unwrap();
    fs::write(fixture.path.join("native.rs"), JSON_ROUTE_NATIVE).unwrap();
    for (name, text, is_high) in [
        ("main.nagi", source, true),
        ("saved.low", low.as_str(), false),
    ] {
        fs::write(fixture.path.join(name), text).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .arg("run")
            .arg(fixture.path.join(name))
            .arg("--project")
            .arg(&fixture.path)
            .arg("--rust")
            .arg(fixture.path.join("native.rs"))
            .arg("--out")
            .arg(fixture.path.join(format!("build-{name}")))
            .env("NAGI_NATIVE_TARGET_DIR", fixture.path.join("native-target"))
            .env("CARGO_NET_OFFLINE", "true")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name} (high={is_high}): stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            "HTTP JSON roundtrip checked",
            "{name}",
        );
    }
}

const JSON_ROUTE_NATIVE: &str = r#"
pub async fn exercise(app: nagi_runtime::http_server::App<super::State, nagi_runtime::Error>) -> Result<(), nagi_runtime::Error> {
    use nagi_runtime::http_server as http;
    use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}, sync::oneshot};
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let options = http::capacity(http::default_options(), 2, 2).unwrap();
    let limits = http::response_limits(1024, 128, 4, 512, 12, 128, 2, 262144, 2, 262144, 512, 12).unwrap();
    let options = http::with_response_limits(options, limits).unwrap();
    let rate_limits = http::rate_limits(4, 1, 2, 131072, 256).unwrap();
    let options = http::with_rate_limits(options, rate_limits).unwrap();
    let options = http::authority(options, "https://localhost", vec!["localhost".to_owned()], 1, 128).unwrap();
    let (stop, shutdown) = oneshot::channel();
    let task = tokio::spawn(http::serve_listener(listener, app, options, async { let _ = shutdown.await; }));
    for (request, expected) in [
        ("POST /data HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 8\r\nConnection: close\r\n\r\n{\"id\":7}", &b"{\"id\":7}"[..]),
        ("GET /optional HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n", &b"null"[..]),
    ] {
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut raw = Vec::new();
        tokio::time::timeout(std::time::Duration::from_secs(3), stream.read_to_end(&mut raw)).await.unwrap().unwrap();
        let separator = raw.windows(4).position(|window| window == b"\r\n\r\n").expect("HTTP response headers");
        let head = std::str::from_utf8(&raw[..separator]).unwrap();
        assert!(head.starts_with("HTTP/1.1 200 "), "{head}");
        assert_eq!(&raw[separator + 4..], expected);
    }
    stop.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), task).await.unwrap().unwrap().unwrap()?;
    Ok(())
}
"#;

#[test]
fn raw_html_stays_a_migration_diagnostic_until_sf04() {
    let source = "import std.http.server as http\nclass State:\n    count: i64\nasync def page(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:\n    return http.html(http.Status.OK, \"<p>Hello</p>\")\ndef setup() -> Result[http.App[State, Error], Error]:\n    app = http.app_default[State](State(count=0))\n    return http.route(app, http.Method.GET, view(\"/page\"), http.public_policy[State](), page)\n";
    rejected_in_high_and_saved_low(source, "SF01 migration: raw HTML");
    let mut high = load_program(source, true);
    let error = check::check(&mut high).unwrap_err();
    assert!(error.contains("SF04"), "{error}");
}

struct Fixture {
    path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi-http-json-roundtrip-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
