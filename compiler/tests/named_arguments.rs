use nagic::{check, emit, parser, source};
use std::{fs, path::PathBuf};

static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(source: &str) -> Self {
        loop {
            let root = std::env::temp_dir().join(format!(
                "nagi-named-arguments-{}-{}",
                std::process::id(),
                ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            match fs::create_dir(&root) {
                Ok(()) => {
                    fs::write(root.join("main.nagi"), source).unwrap();
                    return Self(root);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("cannot reserve fixture {}: {error}", root.display()),
            }
        }
    }

    fn write(&self, name: &str, source: &str) {
        fs::write(self.0.join(name), source).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn checked(source: &str) -> nagic::ast::Program {
    let mut program = parser::parse(source, true).unwrap();
    check::check(&mut program).unwrap();
    program
}

fn diagnostic(source: &str) -> String {
    let mut program = parser::parse(source, true).unwrap();
    check::check(&mut program).unwrap_err()
}

#[test]
fn user_function_named_arguments_lower_to_the_existing_positional_call() {
    let program = checked(
        "def add(left: i64, right: i64) -> i64:\n    return left + right\ndef main():\n    print(add(left=20, right=22))\n",
    );
    let low = emit::low(&program);
    assert!(low.contains("add(20, 22)"), "{low}");
    let mut saved = parser::parse(&low, false).unwrap();
    check::check(&mut saved).unwrap();
}

#[test]
fn class_fields_remain_named_and_may_keep_their_existing_source_order() {
    let program = checked(
        "class Pair:\n    left: i64\n    right: i64\ndef main():\n    pair = Pair(right=22, left=20)\n    print(pair.left + pair.right)\n",
    );
    let low = emit::low(&program);
    assert!(low.contains("Pair(right = 22, left = 20)"), "{low}");
}

#[test]
fn enum_payload_fields_keep_record_semantics() {
    checked(
        "enum Event:\n    Ready(code: i64, label: str)\ndef main():\n    event = Event.Ready(label=\"ok\", code=200)\n    print(1)\n",
    );
}

#[test]
fn builtins_accept_their_published_parameter_name() {
    checked(
        "def main() -> Result[unit, Error]:\n    text = try read_line(prompt=\"name: \")\n    print(value=text)\n    return ok(value=print(value=\"done\"))\n",
    );
}

#[test]
fn standard_response_limits_accepts_all_twelve_names() {
    let fixture = Fixture::new(
        "import std.http.server as http\n\ndef limits() -> Result[http.ResponseLimits, Error]:\n    return http.response_limits(\n        body_bytes=1048576,\n        user_header_bytes=8192,\n        user_header_count=32,\n        managed_header_bytes=8192,\n        managed_header_count=32,\n        serialization_writes=65536,\n        live_responses=64,\n        retained_bytes=83886080,\n        fallback_responses=1024,\n        fallback_retained_bytes=285212672,\n        fallback_managed_header_bytes=8192,\n        fallback_managed_header_count=32\n    )\n",
    );
    let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
    check::check(&mut loaded.program).unwrap();
    let low = emit::low(&loaded.program);
    assert!(
        low.contains(
            "(1048576, 8192, 32, 8192, 32, 65536, 64, 83886080, 1024, 285212672, 8192, 32)"
        ),
        "{low}"
    );
}

#[test]
fn named_argument_diagnostics_are_specific_and_fail_before_lowering() {
    let prefix =
        "def add(left: i64, right: i64) -> i64:\n    return left + right\ndef main():\n    ";
    for (call, expected) in [
        (
            "print(add(left=1, left=2))\n",
            "名前付き引数 left が重複しています",
        ),
        ("print(add(left=1, other=2))\n", "不明な名前付き引数 other"),
        (
            "print(add(left=1))\n",
            "名前付き引数が不足しています: right",
        ),
        (
            "print(add(right=2, left=1))\n",
            "名前付き引数は宣言順に指定してください",
        ),
    ] {
        let error = diagnostic(&format!("{prefix}{call}"));
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn positional_and_named_arguments_do_not_mix() {
    let error = parser::parse(
        "def add(left: i64, right: i64) -> i64:\n    return left + right\ndef main():\n    print(add(1, right=2))\n",
        true,
    )
    .unwrap_err();
    assert!(
        error.contains("位置引数と名前付き引数は混在できません"),
        "{error}"
    );
}

#[test]
fn named_arguments_keep_existing_move_checks() {
    let error = diagnostic(
        "def consume(first: str, second: str) -> i64:\n    return len(view(first)) + len(view(second))\ndef main():\n    first = \"a\"\n    second = \"bb\"\n    print(consume(first=first, second=second))\n    print(first)\n",
    );
    assert!(error.contains("first"), "{error}");
    assert!(error.contains("move") || error.contains("移動"), "{error}");
}

#[test]
fn function_values_require_positional_arguments_because_types_have_no_names() {
    let error = diagnostic(
        "def apply(callback: fn[i64, i64]) -> i64:\n    return callback(value=41)\ndef plus_one(value: i64) -> i64:\n    return value + 1\ndef main():\n    print(apply(plus_one))\n",
    );
    assert!(error.contains("引数名は型に含まれません"), "{error}");
}

#[test]
fn module_resolution_does_not_bypass_a_same_named_local_function_value() {
    let fixture = Fixture::new(
        "def callback(left: i64, right: i64) -> i64:\n    return left + right\ndef apply(callback: fn[i64, i64]) -> i64:\n    return callback(left=20, right=22)\ndef main():\n    print(apply(callback))\n",
    );
    let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
    let error = check::check(&mut loaded.program).unwrap_err();
    assert_eq!(
        error,
        "line 4: 関数値の引数名は型に含まれません。位置引数を使用してください"
    );
}

#[test]
fn module_resolution_does_not_bypass_same_named_local_data() {
    let fixture = Fixture::new(
        "def calculate(value: i64) -> i64:\n    return value + 1\ndef main():\n    calculate = 41\n    print(calculate(value=1))\n",
    );
    let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
    let error = check::check(&mut loaded.program).unwrap_err();
    assert_eq!(error, "line 5: この値は呼び出せる関数ではありません");
}

#[test]
fn existing_constructor_lookup_remains_independent_from_same_named_local_data() {
    let fixture = Fixture::new(
        "class Point:\n    value: i64\ndef main():\n    Point = 41\n    point = Point(value=1)\n    print(Point + point.value)\n",
    );
    let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
    check::check(&mut loaded.program).unwrap();
}

#[test]
fn imported_and_async_functions_share_named_call_normalization() {
    let fixture = Fixture::new(
        "from \"library.nagi\" import add as imported_add\nasync def compute(left: i64, right: i64) -> i64:\n    return imported_add(left=left, right=right)\nasync def main():\n    print(await compute(left=20, right=22))\n",
    );
    fixture.write(
        "library.nagi",
        "def add(left: i64, right: i64) -> i64:\n    return left + right\n",
    );
    let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
    check::check(&mut loaded.program).unwrap();
}

#[test]
fn source_parameter_names_survive_module_binding_renames() {
    let program = checked(
        "def echo(main: i64) -> i64:\n    return main\ndef main():\n    print(echo(main=42))\n",
    );
    let low = emit::low(&program);
    assert!(low.contains("(42)"), "{low}");
    assert!(!low.contains("main = 42"), "{low}");
}

#[test]
fn named_calls_keep_type_errors_and_explain_explicit_type_arguments() {
    let error = diagnostic(
        "def add(left: i64, right: i64) -> i64:\n    return left + right\ndef main():\n    print(add(left=\"wrong\", right=2))\n",
    );
    assert!(error.contains("i64") && error.contains("str"), "{error}");

    let error = parser::parse(
        "def main() -> Result[i64, Error]:\n    return json_decode[i64](input=\"1\")\n",
        true,
    )
    .unwrap_err();
    assert!(
        error.contains("名前付き引数を使う呼出しに明示型引数は指定できません"),
        "{error}"
    );
}
