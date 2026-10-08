use nagic::{check, emit, source};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static SOURCE_ID: AtomicU64 = AtomicU64::new(0);

fn load_program(text: &str, high: bool) -> nagic::ast::Program {
    let extension = if high { "nagi" } else { "low" };
    let path = std::env::temp_dir().join(format!(
        "nagi-http-route-check-{}-{}.{}",
        std::process::id(),
        SOURCE_ID.fetch_add(1, Ordering::Relaxed),
        extension,
    ));
    fs::write(&path, text).unwrap();
    let program = source::load(&path, high).unwrap().program;
    let _ = fs::remove_file(path);
    program
}

fn route_source(routes: &[(&str, &str)]) -> String {
    let mut source = String::from(
        r#"import std.http.server as http
class State:
    count: i64
async def handler(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    return ok(http.text(http.Status.OK, "registered"))
def setup() -> Result[http.App[State, Error], Error]:
    app = http.app_default[State](State(count=0))
"#,
    );
    for (method, path) in routes {
        source.push_str(&format!(
            "    app = try http.route(app, http.Method.{method}, view({path:?}), http.public_policy[State](), handler)\n",
        ));
    }
    source.push_str("    return ok(app)\n");
    source.push_str(
        "def main() -> Result[unit, Error]:\n    app = try setup()\n    return ok(print(\"route registration checked\"))\n",
    );
    source
}

fn checked_high_and_saved_low(source: &str) {
    let mut high = load_program(source, true);
    check::check(&mut high).unwrap();
    let low_text = emit::low(&high);
    let mut low = load_program(&low_text, false);
    check::check(&mut low).unwrap();
    let fixture = Fixture::new();
    for (name, text) in [("main.nagi", source), ("saved.low", low_text.as_str())] {
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
            "{name}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            "route registration checked",
            "{name}"
        );
    }
}

#[test]
fn deprecated_builtin_endpoint_declarations_report_migration_and_paths_are_available() {
    // The former health/stream/websocket endpoints are removed by contract.
    // A finite user route can still register each path; this does not promise
    // implicit health checks, streaming, or websocket behavior.
    for path in ["/health", "/stream", "/ws"] {
        for (source, high) in [
            (
                format!(
                    "@get(\"{path}\")\nasync def custom() -> Result[i64, Error]:\n    return ok(1)\n"
                ),
                true,
            ),
            (
                format!(
                    "@get(\"{path}\")\nasync fn custom() -> Result[i64, Error] {{ return ok(1); }}\n"
                ),
                false,
            ),
        ] {
            let mut program = load_program(&source, high);
            let error = check::check(&mut program).unwrap_err();
            assert!(error.contains("SF01 migration"), "{error}");
            assert!(error.contains("@get/@post/@put/@delete"), "{error}");
            assert!(error.contains("line 2:"), "{error}");
        }
    }

    let source = route_source(&[("GET", "/health"), ("GET", "/stream"), ("GET", "/ws")]);
    checked_high_and_saved_low(&source);
}

fn conflict_matrix_source(cases: &[(&str, &str, &str, &str)]) -> String {
    let mut source = String::from(
        r#"import std.http.server as http
class State:
    count: i64
async def handler(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:
    return ok(http.empty(http.Status.OK))
"#,
    );
    for (index, (left_method, left, right_method, right)) in cases.iter().enumerate() {
        source.push_str(&format!(
            "def conflict_{index}() -> Result[http.App[State, Error], Error]:\n    app = http.app_default[State](State(count=0))\n    app = try http.route(app, http.Method.{}, view({:?}), http.public_policy[State](), handler)\n    return http.route(app, http.Method.{}, view({:?}), http.public_policy[State](), handler)\n",
            left_method.to_ascii_uppercase(),
            left,
            right_method.to_ascii_uppercase(),
            right,
        ));
    }
    source.push_str("def main() -> Result[unit, Error]:\n");
    for (index, _) in cases.iter().enumerate() {
        source.push_str(&format!(
            "    match conflict_{index}():\n        case Ok(_):\n            return error(\"ambiguous route accepted\")\n        case Err(problem):\n            assert_true(error_kind(problem) == \"invalid\")\n            print(error_message(problem))\n"
        ));
    }
    source.push_str("    return ok(print(\"route conflicts checked\"))\n");
    source
}

#[test]
fn runtime_router_rejects_equivalent_capture_paths_in_high_and_saved_low() {
    let cases = [
        ("GET", "/items/{id}", "GET", "/items/{key}"),
        ("GET", "/items/{ba}}r}", "POST", "/items/{id}"),
        (
            "GET",
            "/items/{id}/details/{part}",
            "POST",
            "/items/{key}/details/{part}",
        ),
        ("GET", "/file-{id}", "POST", "/file-{key}"),
        ("GET", "/files/{*tail}", "POST", "/files/{*rest}"),
        ("POST", "/items/{id}", "GET", "/items/{key}"),
        ("POST", "/items/{ba}}r}", "POST", "/items/{id}"),
        (
            "POST",
            "/items/{id}/details/{part}",
            "POST",
            "/items/{key}/details/{part}",
        ),
        ("POST", "/file-{id}", "POST", "/file-{key}"),
        ("POST", "/files/{*tail}", "POST", "/files/{*rest}"),
    ];
    let source = conflict_matrix_source(&cases);
    let mut high = load_program(&source, true);
    check::check(&mut high).unwrap();
    let low = emit::low(&high);
    let fixture = Fixture::new();

    for (name, text) in [("main.nagi", source.as_str()), ("saved.low", low.as_str())] {
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
            "{name}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let messages: Vec<_> = stdout.lines().collect();
        assert_eq!(messages.len(), cases.len() + 1, "{stdout}");
        for (message, (_, left, _, right)) in messages.iter().zip(&cases) {
            assert!(
                message
                    .contains("Insertion failed due to conflict with previously registered route"),
                "{name}: unexpected route rejection: {message}"
            );
            let normalized_left = left.replace("}}", "}");
            assert!(
                message.contains(left)
                    || message.contains(right)
                    || message.contains(&normalized_left),
                "{message}"
            );
        }
        assert_eq!(messages.last(), Some(&"route conflicts checked"));
    }
}

#[test]
fn shared_patterns_distinct_paths_and_literal_braces_remain_available() {
    let source = route_source(&[
        ("GET", "/items/{id}"),
        ("POST", "/items/{id}"),
        ("GET", "/items/list"),
        ("GET", "/files/{*tail}"),
        ("GET", "/literal/{{one}}"),
        ("GET", "/literal/{{two}}"),
        ("POST", "/health"),
    ]);
    checked_high_and_saved_low(&source);
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
            "nagi-http-route-conflicts-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
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
