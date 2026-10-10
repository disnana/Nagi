#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, parser, source};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static SOURCE_ID: AtomicU64 = AtomicU64::new(0);

fn checked(source: &str, high: bool) -> nagic::ast::Program {
    let extension = if high { "nagi" } else { "low" };
    let path = std::env::temp_dir().join(format!(
        "nagi-http-entrypoint-check-{}-{}.{}",
        std::process::id(),
        SOURCE_ID.fetch_add(1, Ordering::Relaxed),
        extension,
    ));
    fs::write(&path, source).unwrap();
    let mut loaded = source::load(&path, high).unwrap();
    check::check(&mut loaded.program)
        .unwrap_or_else(|error| panic!("{}\n{source}", loaded.diagnostic(&error)));
    let _ = fs::remove_file(path);
    loaded.program
}

#[test]
fn legacy_global_serve_calls_in_nested_high_and_saved_low_report_migration() {
    let bodies = [
        "    db = 0\n    return await serve(db, -1)\n",
        "    db = 0\n    result = await serve(db, -1)\n    return result\n",
        "    db = 0\n    return ok(try await serve(db, -1))\n",
        "    db = 0\n    try await serve(db, -1)\n    return ok(print(1))\n",
        "    db = 0\n    if True:\n        return await serve(db, -1)\n    else:\n        return ok(print(1))\n",
        "    db = 0\n    if False:\n        return ok(print(1))\n    else:\n        return await serve(db, -1)\n",
        "    db = 0\n    match parse_i64(\"1\"):\n        case Ok(_):\n            return await serve(db, -1)\n        case Err(problem):\n            return fail(problem)\n",
        "    while False:\n        db = 0\n        try await serve(db, -1)\n    return ok(print(1))\n",
        "    for port in range(1):\n        db = 0\n        try await serve(db, port)\n    return ok(print(1))\n",
        "    db = 0\n    async with scope:\n        spawn serve(db, -1)\n    return ok(print(1))\n",
        "    db = 0\n    results: List[Result[unit, Error]] = [await serve(db, -1)]\n    return ok(print(1))\n",
    ];
    for body in bodies {
        let source = format!("async def start() -> Result[unit, Error]:\n{body}");
        let parsed = parser::parse(&source, true).unwrap();
        let saved_low = emit::low(&parsed);
        for (text, high) in [(&source, true), (&saved_low, false)] {
            let mut program = parser::parse(text, high).unwrap();
            let error = check::check(&mut program).unwrap_err();
            assert!(error.contains("SF01 migration"), "{error}\n{text}");
            assert!(error.contains("旧serve/html"), "{error}\n{text}");
            let serve_line = text
                .lines()
                .position(|line| line.contains("serve("))
                .unwrap()
                + 1;
            assert!(
                error.contains(&format!("line {serve_line}:")),
                "{error}\n{text}"
            );
        }
    }
}

#[test]
fn shadowed_serve_calls_and_non_http_programs_do_not_emit_http_glue() {
    let sources = [
        "def serve(value: i64, port: i64) -> i64:\n    return value + port\ndef answer() -> i64:\n    return serve(40, 2)\n",
        "def add(value: i64, port: i64) -> i64:\n    return value + port\ndef answer() -> i64:\n    serve = add\n    return serve(40, 2)\n",
        "def answer() -> i64:\n    return 42\n",
    ];
    for source in sources {
        let high = checked(source, true);
        let low = checked(&emit::low(&high), false);
        for program in [&high, &low] {
            let rust = emit::rust(&checked_emission::seal(program)).unwrap();
            assert!(!rust.contains("async fn __nagi_serve("), "{source}");
            assert!(!rust.contains("::nagi_runtime"), "{source}");
        }
    }
}

#[test]
fn explicit_routes_without_serve_do_not_synthesize_an_entrypoint() {
    let source = "import std.http.server as http\nclass State:\n    value: i64\nasync def answer(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:\n    return ok(http.text(http.Status.OK, \"42\"))\ndef setup() -> Result[http.App[State, Error], Error]:\n    app = http.app_default[State](State(value=0))\n    return http.route(app, http.Method.GET, view(\"/answer\"), http.public_policy[State](), answer)\n";
    let high = checked(source, true);
    let low = checked(&emit::low(&high), false);
    let handler = high
        .modules
        .resolve_root_path("answer")
        .unwrap()
        .symbol
        .clone();
    for program in [&high, &low] {
        let rust = emit::rust(&checked_emission::seal(program)).unwrap();
        assert!(!rust.contains("async fn __nagi_serve("));
        assert!(rust.contains("::nagi_runtime::http_server::route("));
        assert!(
            rust.contains("::nagi_runtime::http_server::public_policy"),
            "{rust}"
        );
        assert!(rust.contains(&format!("crate::{handler}")), "{rust}");
    }
}

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn explicit_http_server_builds_and_returns_invalid_port_in_high_and_low() {
    // An invalid port exercises explicit App/route/serve through the real
    // generated entrypoint without opening a socket or leaving a server live.
    let source = "import std.http.server as http\nclass State:\n    value: i64\nasync def handler(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, Error]:\n    return ok(http.empty(http.Status.OK))\nasync def main() -> Result[unit, Error]:\n    app = http.app_default[State](State(value=0))\n    app = try http.route(app, http.Method.GET, view(\"/\"), http.public_policy[State](), handler)\n    if True:\n        match await http.serve(app, -1, http.default_options()):\n            case Ok(_):\n                return error(\"invalid port accepted\")\n            case Err(problem):\n                assert_true(error_kind(problem) == \"invalid\")\n                return ok(print(\"invalid port handled\"))\n    else:\n        return error(\"wrong branch\")\n";
    let high = checked(source, true);
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "nagi-http-entrypoint-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )));
    fs::create_dir_all(&fixture.0).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("native-target"));
    for (name, source) in [
        ("empty.nagi", source.to_owned()),
        ("empty.low", emit::low(&high)),
    ] {
        fs::write(fixture.0.join(name), source).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(&fixture.0)
            .args(["run", name, "--no-project"])
            .env("NAGI_ROOT", &root)
            .env("NAGI_NATIVE_TARGET_DIR", &target)
            .env("CARGO_NET_OFFLINE", "true")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).lines().last(),
            Some("invalid port handled")
        );
    }
}
