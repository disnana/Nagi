#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{ast::Program, check, emit, parser, source};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn checked(text: &str) -> (Self, Program) {
        let path = std::env::temp_dir().join(format!(
            "nagi-http-codegen-{}-{}",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("main.nagi"), text).unwrap();
        let fixture = Self(path);
        let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
        check::check(&mut loaded.program)
            .unwrap_or_else(|error| panic!("{}", loaded.diagnostic(&error)));
        (fixture, loaded.program)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn independent_low(program: &Program) -> Program {
    let low = emit::low(program);
    let mut saved = parser::parse(&low, false).unwrap();
    check::check(&mut saved).unwrap_or_else(|error| panic!("{error}\n{low}"));
    saved
}

#[test]
fn checked_authority_configuration_uses_registry_in_high_saved_and_handwritten_low() {
    let high = "import std.http.server as http\ndef configure() -> Result[http.Options, Error]:\n    limits = http.default_options()\n    limits = try http.authority(limits, \"https://localhost\", [\"localhost\", \"localhost:443\"], 4, 256)\n    return http.trusted_proxy(limits, [\"127.0.0.1\"])\n";
    let low = "import std.http.server as http;\nfn configure() -> Result[http.Options, Error] {\n    let limits = http.default_options();\n    limits = try http.authority(limits, \"https://localhost\", [\"localhost\", \"localhost:443\"], 4, 256);\n    return http.trusted_proxy(limits, [\"127.0.0.1\"]);\n}\n";
    let (fixture, program) = Fixture::checked(high);
    let hand_path = fixture.0.join("hand.low");
    fs::write(&hand_path, low).unwrap();
    let mut loaded = source::load(&hand_path, false).unwrap();
    check::check(&mut loaded.program)
        .unwrap_or_else(|error| panic!("{}", loaded.diagnostic(&error)));
    let hand = loaded.program;
    for checked in [&program, &independent_low(&program), &hand] {
        let rust = emit::rust(&checked_emission::seal(checked)).unwrap();
        assert!(rust.contains("::nagi_runtime::http_server::authority("));
        assert!(rust.contains("::nagi_runtime::http_server::trusted_proxy("));
        for name in ["authority", "trusted_proxy"] {
            let definition = checked
                .modules
                .resolve_root_path(&format!("http.{name}"))
                .unwrap();
            assert_eq!(definition.id.module.0, "stdlib:std.http.server");
        }
    }
}

fn explicit_policy_source(source: &str) -> String {
    let source = source.replace(
        "async def handler(request: http.Request, state: shared[State])",
        "async def handler(request: http.Request, state: shared[State], authority: unit)",
    );
    let source = source.replace(
        "http.route(app, http.Method.GET, view(\"/probe\"), handler)",
        "http.route(app, http.Method.GET, view(\"/probe\"), http.public_policy[State](), handler)",
    );
    source.replace(
        "http.route_mapped(app, http.Method.GET, view(\"/probe\"), handler, map_text)",
        "http.route_mapped(app, http.Method.GET, view(\"/probe\"), http.public_policy[State](), handler, map_text)",
    )
}

// This whole handler reads native request/header/body views and shared state.
// It needs no owned string construction, state clone, or adapter allocation.
// Response construction and the heterogeneous runtime route table have their
// own measured costs; this checks that code generation adds none of those.
const AUTH: &str = include_str!("fixtures/resource-contract/http-inspection.nagi");

#[test]
fn borrowed_http_inspection_and_async_registration_add_no_owned_adapter_work() {
    let (_fixture, program) = Fixture::checked(&explicit_policy_source(AUTH));
    let saved = independent_low(&program);
    let high_rust = emit::rust(&checked_emission::seal(&program)).unwrap();
    let low_rust = emit::rust(&checked_emission::seal(&saved)).unwrap();
    assert_eq!(high_rust, low_rust, "saved Low changed adapter work");

    for operation in [
        "String::from(",
        ".to_owned()",
        ".clone()",
        "Arc::clone(",
        "Arc::new(",
        "Box::",
        "BoxFuture",
        "format!(",
    ] {
        assert!(
            !high_rust.contains(operation),
            "borrowed HTTP adapter introduced {operation}:\n{high_rust}"
        );
    }
    assert!(high_rust.contains("&'a ::nagi_runtime::http_server::Request"));
    assert!(high_rust.contains("::nagi_runtime::http_server::Method::GET"));
    assert!(high_rust.contains("::nagi_runtime::http_server::Status::OK"));
    assert!(high_rust.contains("::nagi_runtime::http_server::header_text("));
    let handler = &program.modules.resolve_root_path("handler").unwrap().symbol;
    let registration = high_rust
        .lines()
        .find(|line| line.contains("::nagi_runtime::http_server::route("))
        .expect("missing native handler registration");
    assert!(registration.contains(&format!(", crate::{handler})")));
}

#[test]
fn literal_view_optimization_does_not_change_a_shadowing_owned_function() {
    let (_fixture, program) = Fixture::checked(
        "def view(value: str) -> str:\n    return value\ndef owned() -> str:\n    return view(\"owned argument\")\n",
    );
    let function = &program.modules.resolve_root_path("view").unwrap().symbol;
    for checked in [&program, &independent_low(&program)] {
        let rust = emit::rust(&checked_emission::seal(checked)).unwrap();
        assert!(rust.contains(&format!(
            "crate::{function}(::std::string::String::from(\"owned argument\"))"
        )));
    }
}

const BORROW_MAPPERS: &str = include_str!("fixtures/resource-contract/http-borrow-mappers.nagi");

#[test]
fn borrow_and_named_mappers_preserve_high_low_codegen() {
    let (_fixture, program) = Fixture::checked(&explicit_policy_source(BORROW_MAPPERS));
    let high_rust = emit::rust(&checked_emission::seal(&program)).unwrap();
    let low_rust = emit::rust(&checked_emission::seal(&independent_low(&program))).unwrap();
    assert_eq!(
        high_rust, low_rust,
        "saved Low changed Borrow/Mapper emission"
    );
    assert!(high_rust.contains("::nagi_runtime::http_server::json::<"));
    assert!(
        high_rust.contains(", &(value))"),
        "json must borrow its owned DTO: {high_rust}"
    );
    for (operation, callback) in [("app::<", "map_builtin"), ("route_mapped(", "map_text")] {
        let callback = &program.modules.resolve_root_path(callback).unwrap().symbol;
        let line = high_rust
            .lines()
            .find(|line| line.contains(&format!("::nagi_runtime::http_server::{operation}")))
            .unwrap();
        assert!(
            line.contains(&format!("crate::{callback}")),
            "named mapper lost: {line}"
        );
    }
}

#[test]
fn borrow_and_named_mapper_registration_execute_native_adapters() {
    use std::process::Command;
    // Run JSON/Borrow from the response context of a real bounded request, and
    // retain both named mapper registrations through High and saved Low.
    let mapped = explicit_policy_source(BORROW_MAPPERS)
        .replace(
            "match http.empty(http.response_context(view(request)), http.Status.OK):",
            "match encode(http.response_context(view(request)), Payload(label=\"seven\")):",
        )
        .replace(
            "return http.route_mapped(app, http.Method.GET, view(\"/probe\"), http.public_policy[State](), handler, map_text)",
            "app = try http.route_mapped(app, http.Method.GET, view(\"/probe\"), http.public_policy[State](), handler, map_text)\n    rate = try http.rate_policy(10, 1000)\n    return http.rate_route(app, http.Method.GET, \"/probe\", rate)",
        );
    let source = format!(
        "{}\n@rust(\"native::probe\")\nextern async def probe() -> Result[unit, Error]\nasync def main() -> Result[unit, Error]:\n    return await probe()\n",
        mapped
    );
    let (fixture, program) = Fixture::checked(&source);
    fs::write(fixture.0.join("saved.low"), emit::low(&program)).unwrap();
    fs::write(
        fixture.0.join("nagi.toml"),
        "[rust.dependencies]\ntokio = { version = \"1.48\", features = [\"net\", \"io-util\", \"sync\", \"time\", \"rt\"] }\n",
    )
    .unwrap();
    fs::write(
        fixture.0.join("native.rs"),
        r#"
pub async fn probe() -> Result<(), nagi_runtime::Error> {
    use nagi_runtime::http_server as http;
    use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}, sync::oneshot};
    use std::time::Duration;
    let app = crate::setup(crate::State { seed: 1 })?;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let options = http::capacity(http::default_options(), 1, 1)?;
    let limits = http::response_limits(1024, 128, 4, 512, 12, 128, 1, 131072, 1, 131072, 512, 12)?;
    let options = http::with_response_limits(options, limits)?;
    let rate_limits = http::rate_limits(4, 1, 2, 131072, 64)?;
    let options = http::with_rate_limits(options, rate_limits)?;
    let options = http::authority(options, "https://localhost", vec!["localhost".to_owned()], 1, 128)?;
    let (stop, shutdown) = oneshot::channel();
    let task = tokio::spawn(http::serve_listener(listener, app, options, async { let _ = shutdown.await; }));
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream.write_all(b"GET /probe HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").await.unwrap();
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(3), stream.read_to_end(&mut response)).await.unwrap().unwrap();
    let separator = response.windows(4).position(|window| window == b"\r\n\r\n").unwrap();
    let head = std::str::from_utf8(&response[..separator]).unwrap();
    assert!(head.starts_with("HTTP/1.1 200 "), "{head}");
    assert_eq!(&response[separator + 4..], b"{\"label\":\"seven\"}");
    stop.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(3), task).await.unwrap().unwrap()?;
    println!("borrow and named mapper registration execute");
    Ok(())
}
"#,
    )
    .unwrap();
    let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("native-target")
        });
    for input in ["main.nagi", "saved.low"] {
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(&fixture.0)
            .args([
                "run",
                input,
                "--rust",
                "native.rs",
                "--out",
                "build",
                "--project",
                fixture.0.to_str().unwrap(),
            ])
            .env("NAGI_NATIVE_TARGET_DIR", &target)
            .env("CARGO_NET_OFFLINE", "true")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            "borrow and named mapper registration execute",
            "{input}"
        );
        let stderr = String::from_utf8(output.stderr).unwrap();
        let native = stderr
            .lines()
            .find_map(|line| line.strip_prefix("native: "))
            .unwrap()
            .trim_end_matches('\r');
        assert!(std::path::Path::new(native).is_file());
        if input == "main.nagi" {
            fs::remove_file(fixture.0.join(input)).unwrap();
        }
    }
}
