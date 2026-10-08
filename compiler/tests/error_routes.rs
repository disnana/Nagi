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
            "import std.http.server as http\n{declaration}class State:\n    count: i64\nasync def handler(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:\n    decoded = try json_decode[Private](request.body)\n    return ok(http.empty(http.Status.OK))\ndef setup() -> Result[http.App[State, Error], Error]:\n    app = http.app_default[State](State(count=0))\n    return http.route(app, http.Method.POST, view(\"/input\"), http.public_policy[State](), handler)\n"
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
                "import std.http.server as http\n{declaration}class State:\n    count: i64\nasync def handler(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:\n    return ok(http.empty(http.Status.OK))\ndef setup() -> Result[http.App[State, Error], Error]:\n    app = http.app_default[State](State(count=0))\n    app = try http.route(app, http.Method.GET, view(\"/output\"), http.public_policy[State](), handler)\n    return ok(app)\ndef encode({parameter}) -> Result[http.Response, Error]:\n    return http.json[{json_type}](http.Status.OK, {})\n",
                if parameter.starts_with("values:") { "values" } else { "value" },
            );
            rejected_in_high_and_saved_low(
                &output,
                "HTTP JSON builderの値はSerializeに対応していません",
            );
        }
    }

    let direct_enum = "import std.http.server as http\nenum Failure:\n    Missing\nclass State:\n    count: i64\nasync def handler(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:\n    return ok(http.empty(http.Status.OK))\ndef setup() -> Result[http.App[State, Error], Error]:\n    app = http.app_default[State](State(count=0))\n    app = try http.route(app, http.Method.GET, view(\"/output\"), http.public_policy[State](), handler)\n    return ok(app)\ndef encode(value: Failure) -> Result[http.Response, Error]:\n    return http.json[Failure](http.Status.OK, value)\n";
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
    input = try json_decode[Input](request.body)
    return http.json[Output](http.Status.OK, Output(id=input.id))
async def optional(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    return http.json[Output?](http.Status.OK, None)
def setup() -> Result[http.App[State, Error], Error]:
    app = http.app_default[State](State(count=0))
    app = try http.route(app, http.Method.POST, view("/data"), http.public_policy[State](), data)
    return http.route(app, http.Method.GET, view("/optional"), http.public_policy[State](), optional)
def main() -> Result[unit, Error]:
    app = try setup()
    input = try json_decode[Input]("{\"id\":7}")
    response = try http.json[Output](http.Status.OK, Output(id=input.id))
    roundtrip = try json_decode[Output](response.body)
    assert_true(roundtrip.id == 7)
    absent = try http.json[Output?](http.Status.OK, None)
    assert_true(len(absent.body) == 4)
    decoded_absent = try json_decode[Output?](absent.body)
    match decoded_absent:
        case None:
            return ok(print("HTTP JSON roundtrip checked"))
        case Some(_):
            return error("optional null changed representation")
"#;
    let mut high = load_program(source, true);
    check::check(&mut high).unwrap();
    let low = emit::low(&high);
    let fixture = Fixture::new();
    for (name, text, is_high) in [
        ("main.nagi", source, true),
        ("saved.low", low.as_str(), false),
    ] {
        fs::write(fixture.path.join(name), text).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(&fixture.path)
            .args(["run", name, "--no-project"])
            .env("NAGI_ROOT", &fixture.root)
            .env("NAGI_NATIVE_TARGET_DIR", &fixture.target)
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

#[test]
fn raw_html_stays_a_migration_diagnostic_until_sf04() {
    let source = "import std.http.server as http\nclass State:\n    count: i64\nasync def page(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:\n    return http.html(http.Status.OK, \"<p>Hello</p>\")\ndef setup() -> Result[http.App[State, Error], Error]:\n    app = http.app_default[State](State(count=0))\n    return http.route(app, http.Method.GET, view(\"/page\"), http.public_policy[State](), page)\n";
    rejected_in_high_and_saved_low(source, "SF01 migration: raw HTML");
    let mut high = load_program(source, true);
    let error = check::check(&mut high).unwrap_err();
    assert!(error.contains("SF04"), "{error}");
}

struct Fixture {
    root: PathBuf,
    target: PathBuf,
    path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("native-target"));
        let path = std::env::temp_dir().join(format!(
            "nagi-http-json-roundtrip-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { root, target, path }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
