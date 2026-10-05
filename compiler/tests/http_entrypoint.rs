#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, parser};
use std::{fs, path::PathBuf, process::Command};

fn checked(source: &str, high: bool) -> nagic::ast::Program {
    let mut program = parser::parse(source, high).unwrap();
    check::check(&mut program).unwrap_or_else(|error| panic!("{error}\n{source}"));
    program
}

#[test]
fn zero_route_entrypoint_follows_builtin_calls_in_nested_high_and_low() {
    let bodies = [
        "    db = try await db_open(\":memory:\")\n    return await serve(db, -1)\n",
        "    db = try await db_open(\":memory:\")\n    result = await serve(db, -1)\n    return result\n",
        "    db = try await db_open(\":memory:\")\n    return ok(try await serve(db, -1))\n",
        "    db = try await db_open(\":memory:\")\n    try await serve(db, -1)\n    return ok(print(1))\n",
        "    db = try await db_open(\":memory:\")\n    if True:\n        return await serve(db, -1)\n    else:\n        return ok(print(1))\n",
        "    db = try await db_open(\":memory:\")\n    if False:\n        return ok(print(1))\n    else:\n        return await serve(db, -1)\n",
        "    db = try await db_open(\":memory:\")\n    match parse_i64(\"1\"):\n        case Ok(_):\n            return await serve(db, -1)\n        case Err(problem):\n            return fail(problem)\n",
        "    while False:\n        db = try await db_open(\":memory:\")\n        try await serve(db, -1)\n    return ok(print(1))\n",
        "    for port in range(1):\n        db = try await db_open(\":memory:\")\n        try await serve(db, port)\n    return ok(print(1))\n",
        "    db = try await db_open(\":memory:\")\n    async with scope:\n        spawn serve(db, -1)\n    return ok(print(1))\n",
        "    db = try await db_open(\":memory:\")\n    results: List[Result[unit, Error]] = [await serve(db, -1)]\n    return ok(print(1))\n",
    ];
    for body in bodies {
        let high = checked(
            &format!("async def start() -> Result[unit, Error]:\n{body}"),
            true,
        );
        let low = checked(&emit::low(&high), false);
        for program in [&high, &low] {
            let rust = emit::rust(&checked_emission::seal(program)).unwrap();
            assert!(rust.contains("async fn __nagi_serve("), "{body}");
            assert!(!rust.contains("async fn __route_"), "{body}");
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
fn routes_without_a_serve_call_still_emit_http_glue() {
    let program = checked(
        "@get(\"/answer\")\nasync def answer() -> Result[i64, Error]:\n    return ok(42)\n",
        true,
    );
    let rust = emit::rust(&checked_emission::seal(&program)).unwrap();
    assert!(rust.contains("async fn __nagi_serve("));
    assert!(rust.contains("async fn __route_0("));
}

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn zero_route_servers_build_and_return_runtime_errors_in_high_and_low() {
    // An invalid port exercises the real generated entrypoint without opening
    // a socket or leaving a long-running server behind.
    let source = "def forward(result: Result[unit, Error]) -> Result[unit, Error]:\n    return result\nasync def main() -> Result[unit, Error]:\n    db = try await db_open(\":memory:\")\n    if True:\n        match forward(await serve(db, -1)):\n            case Ok(_):\n                return error(\"invalid port accepted\")\n            case Err(problem):\n                assert_true(error_kind(problem) == \"invalid\")\n                return ok(print(\"invalid port handled\"))\n    else:\n        return error(\"wrong branch\")\n";
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
