use nagic::{check, emit, parser, source};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);
static NATIVE_RUN: Mutex<()> = Mutex::new(());

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi typed errors {} {}",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn checked(&self, name: &str) -> source::Sources {
        let mut loaded = source::load(&self.0.join(name), name.ends_with(".nagi"))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        check::check(&mut loaded.program)
            .unwrap_or_else(|error| panic!("{}", loaded.diagnostic(&error)));
        loaded
    }

    fn roundtrip(&self, name: &str) -> source::Sources {
        let loaded = self.checked(name);
        let identities = |program: &nagic::ast::Program| {
            program
                .modules
                .definitions
                .iter()
                .map(|definition| (definition.id.clone(), definition.symbol.clone()))
                .collect::<Vec<_>>()
        };
        let expected = identities(&loaded.program);
        let low = emit::low(&loaded.program);
        let mut independent = parser::parse(&low, false).unwrap();
        check::check(&mut independent)
            .unwrap_or_else(|error| panic!("independent Low: {error}\n{low}"));
        assert_eq!(identities(&independent), expected);
        self.write("saved.low", &low);
        let saved = self.checked("saved.low");
        assert_eq!(identities(&saved.program), expected);
        saved
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }

    fn run_high_and_saved_low(&self, expected: &str) {
        self.run_high_and_saved_low_with_args(expected, &[]);
    }

    fn run_high_and_saved_low_with_args(&self, expected: &str, extra: &[&str]) {
        let _native_run = NATIVE_RUN
            .lock()
            .expect("the shared native build/run lock must remain unpoisoned");
        let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join("native-target")
            });
        for source in ["main.nagi", "saved.low"] {
            let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
                .current_dir(&self.0)
                .args(["run", source, "--out", "build"])
                .args(extra)
                .env("NAGI_NATIVE_TARGET_DIR", &target)
                .env("CARGO_NET_OFFLINE", "true")
                .output()
                .unwrap();
            let stderr = String::from_utf8(output.stderr.clone()).unwrap();
            let launcher = stderr
                .lines()
                .find(|line| line.starts_with("native: "))
                .expect("run must report the native executable on stderr");
            let application = successful(output);
            let binary = launcher
                .trim_end_matches('\r')
                .strip_prefix("native: ")
                .expect("the status line must identify the launched native executable");
            let binary = Path::new(binary);
            assert!(
                binary.is_file(),
                "reported executable must exist: {launcher}"
            );
            let expected_target = self.0.join(&target).join("release");
            assert_eq!(binary.parent(), Some(expected_target.as_path()));
            assert_eq!(application.trim(), expected, "{source}");
        }
    }

    fn rejected(&self, text: &str, diagnostic_name: &str) {
        self.write("main.nagi", text);
        let error = match source::load(&self.0.join("main.nagi"), true) {
            Err(error) => error,
            Ok(mut loaded) => {
                let error = check::check(&mut loaded.program)
                    .expect_err("invalid typed error program passed the checker");
                loaded.diagnostic(&error)
            }
        };
        assert!(error.contains("main.nagi:"), "{error}");
        assert!(error.contains(diagnostic_name), "{text}\n{error}");
        let output = self.cli(&["check", "main.nagi", "--out", "rejected"]);
        assert!(!output.status.success(), "CLI accepted {text}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(diagnostic_name),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn successful(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn class_errors_keep_context_nested_results_same_error_try_and_explicit_conversion() {
    let f = Fixture::new();
    f.write(
        "main.nagi",
        r#"class Err:
    value: i64
class AppError:
    code: i64
def rejected() -> Result[i64, Err]:
    return fail(Err(value=7))
def propagated() -> Result[i64, Err]:
    value = try rejected()
    return ok(value + 1)
def nested() -> Result[Result[i64, Err], Err]:
    return ok(fail(Err(value=11)))
def optional() -> Result[i64, Err]?:
    return some(fail(Err(value=13)))
def standalone() -> Result[unit, Err]:
    outcome = fail(Err(value=21))
    return outcome
def convert(error: Err) -> AppError:
    return AppError(code=error.value + 100)
def converted() -> Result[i64, AppError]:
    match propagated():
        case Ok(value):
            return ok(value)
        case Err(error):
            return fail(convert(error))
def inspect() -> i64:
    match converted():
        case Ok(value):
            return value
        case Err(error):
            return error.code
def inspect_nested() -> i64:
    match nested():
        case Ok(inner):
            match inner:
                case Ok(value):
                    return value
                case Err(error):
                    return error.value
        case Err(error):
            return error.value
def inspect_standalone() -> i64:
    match standalone():
        case Ok(_):
            return 0
        case Err(error):
            return error.value
def main():
    assert_true(inspect() == 107)
    assert_true(inspect_nested() == 11)
    assert_true(inspect_standalone() == 21)
    print("class errors preserved")
"#,
    );
    f.roundtrip("main.nagi");
    f.run_high_and_saved_low("class errors preserved");
}

#[test]
fn enums_construct_match_owned_payloads_and_preserve_copy_variants() {
    let f = Fixture::new();
    f.write(
        "main.nagi",
        r#"enum AuthError:
    InvalidCredentials
    UserExists
    WeakPassword(message: str)
    Pair(first: i64, second: str)
enum Count:
    Empty
    Value(number: i64)
def failed() -> Result[unit, AuthError]:
    return fail(AuthError.WeakPassword(message="short"))
def describe(error: AuthError) -> str:
    match error:
        case AuthError.InvalidCredentials:
            return "invalid"
        case AuthError.UserExists:
            return "exists"
        case AuthError.WeakPassword(message):
            return message
        case AuthError.Pair(first, second):
            return second
def code(error: AuthError) -> i64:
    match error:
        case AuthError.InvalidCredentials:
            return 1
        case AuthError.UserExists:
            return 2
        case AuthError.WeakPassword(message):
            return len(view(message))
        case AuthError.Pair(first, _):
            return first
def count(value: Count) -> i64:
    match value:
        case Count.Empty:
            return 0
        case Count.Value(number):
            return number
def count_twice(value: Count) -> i64:
    first = count(value)
    return first + count(value)
def inspect() -> str:
    match failed():
        case Ok(_):
            return "unexpected"
        case Err(error):
            return describe(error)
def main():
    assert_true(describe(AuthError.InvalidCredentials) == "invalid")
    assert_true(describe(AuthError.UserExists) == "exists")
    assert_true(describe(AuthError.WeakPassword("short")) == "short")
    assert_true(describe(AuthError.Pair(first=7, second="owned")) == "owned")
    assert_true(code(AuthError.Pair(7, "ignored")) == 7)
    assert_true(code(AuthError.WeakPassword("five!")) == 5)
    assert_true(count_twice(Count.Value(4)) == 8)
    assert_true(inspect() == "short")
    print("enum errors preserved")
"#,
    );
    f.roundtrip("main.nagi");
    f.run_high_and_saved_low("enum errors preserved");
}

#[test]
fn async_custom_errors_propagate_the_same_enum_through_await_and_try() {
    let f = Fixture::new();
    f.write(
        "main.nagi",
        r#"enum AsyncFailure:
    Offline
    Retry(after: i64)
async def failed() -> Result[i64, AsyncFailure]:
    return fail(AsyncFailure.Retry(after=7))
async def propagated() -> Result[i64, AsyncFailure]:
    value = try await failed()
    return ok(value + 1)
async def main():
    match await propagated():
        case Ok(value):
            assert_true(false)
        case Err(error):
            match error:
                case AsyncFailure.Offline:
                    assert_true(false)
                case AsyncFailure.Retry(after):
                    assert_true(after == 7)
    print("async errors preserved")
"#,
    );
    f.roundtrip("main.nagi");
    f.run_high_and_saved_low("async errors preserved");
}

#[test]
fn scope_enum_conversion_requires_the_explicit_rust_bridge_and_keeps_the_owned_cause() {
    let f = Fixture::new();
    f.write(
        "main.nagi",
        r#"enum ScopeFailure:
    Runtime(cause: Error)
async def child() -> Result[unit, Error]:
    return error("task failed")
async def scoped() -> Result[unit, ScopeFailure]:
    async with scope:
        spawn child()
    return ok(print("unexpected success"))
async def main():
    match await scoped():
        case Ok(_):
            assert_true(false)
        case Err(failure):
            match failure:
                case ScopeFailure.Runtime(cause):
                    assert_true(error_message(cause) == "task failed")
    print("scope enum conversion preserved")
"#,
    );
    f.write(
        "bridge.rs",
        r#"impl From<nagi_runtime::Error> for super::ScopeFailure {
    fn from(cause: nagi_runtime::Error) -> Self {
        Self::Runtime { cause }
    }
}
"#,
    );
    f.roundtrip("main.nagi");
    f.run_high_and_saved_low_with_args("scope enum conversion preserved", &["--rust", "bridge.rs"]);
}

#[test]
fn module_aliases_share_enum_error_identity_and_keep_qualified_variants() {
    let f = Fixture::new();
    f.write(
        "errors.nagi",
        r#"enum Failure:
    Missing
    Detail(message: str)
def failed() -> Result[i64, Failure]:
    return fail(Failure.Detail("orders"))
"#,
    );
    f.write(
        "main.nagi",
        r#"import "errors.nagi" as errors
import "./errors.nagi" as again
from "errors.nagi" import Failure as SavedFailure
def identity(error: SavedFailure) -> errors.Failure:
    return error
def describe(error: again.Failure) -> str:
    match error:
        case SavedFailure.Missing:
            return "missing"
        case errors.Failure.Detail(message):
            return message
def propagated() -> Result[i64, SavedFailure]:
    value = try errors.failed()
    return ok(value)
def inspect() -> str:
    match propagated():
        case Ok(value):
            return "unexpected"
        case Err(error):
            return describe(identity(error))
def main():
    assert_true(describe(identity(again.Failure.Missing)) == "missing")
    assert_true(describe(SavedFailure.Detail(message="saved")) == "saved")
    assert_true(inspect() == "orders")
    print("module error identity preserved")
"#,
    );
    let saved = f.roundtrip("main.nagi");
    let error = saved
        .program
        .modules
        .resolve_root_path("errors.Failure")
        .unwrap();
    assert_eq!(
        error.id,
        saved
            .program
            .modules
            .resolve_root_path("again.Failure")
            .unwrap()
            .id
    );
    assert_eq!(
        error.id,
        saved
            .program
            .modules
            .resolve_root_path("SavedFailure")
            .unwrap()
            .id
    );
    f.run_high_and_saved_low("module error identity preserved");
}

#[test]
fn builtin_error_causes_survive_wrapping_without_json_or_database_requirements() {
    let f = Fixture::new();
    f.write(
        "main.nagi",
        r#"class InputError:
    cause: Error
    message: str
def parse_input() -> Result[i64, InputError]:
    match parse_i64("not-an-integer"):
        case Ok(value):
            return ok(value)
        case Err(cause):
            return fail(InputError(cause=cause, message="invalid input"))
def main():
    match parse_input():
        case Ok(value):
            assert_true(false)
        case Err(error):
            assert_true(error.message == "invalid input")
            assert_true(error_kind(error.cause) == "invalid")
            assert_true(len(error_message(error.cause)) > 0)
    print("diagnostic cause preserved")
"#,
    );
    f.roundtrip("main.nagi");
    f.run_high_and_saved_low("diagnostic cause preserved");
}

#[test]
fn fail_and_try_reject_implicit_conversion_between_custom_error_types() {
    let f = Fixture::new();
    let classes = "class First:\n    value: i64\nclass Second:\n    value: i64\n";
    for body in [
        "def bad() -> Result[i64, First]:\n    return fail(Second(value=1))\n",
        "def failed() -> Result[i64, First]:\n    return fail(First(value=1))\ndef bad() -> Result[i64, Second]:\n    value = try failed()\n    return ok(value)\n",
        "def bad() -> Result[Result[i64, First], First]:\n    return ok(fail(Second(value=1)))\n",
        "def bad(cause: Error) -> Result[i64, First]:\n    return fail(cause)\n",
    ] {
        f.rejected(&format!("{classes}{body}"), "First");
    }
}

#[test]
fn diagnostic_cause_containers_are_rejected_by_json_serialization() {
    let f = Fixture::new();
    let definition = "class InputError:\n    cause: Error\n    message: str\n";
    for body in [
        "def bad(cause: Error) -> Result[str, Error]:\n    return json_encode(InputError(cause=cause, message=\"public\"))\n",
        "def bad(cause: Error) -> Result[str, Error]:\n    values = [InputError(cause=cause, message=\"public\")]\n    return json_encode(values)\n",
        "def bad(text: view[str]) -> Result[InputError, Error]:\n    return json_decode[InputError](text)\n",
    ] {
        f.rejected(&format!("{definition}{body}"), "InputError");
    }
}

#[test]
fn enum_constructors_reject_unknown_variants_wrong_arity_and_wrong_payload_types() {
    let f = Fixture::new();
    let definition =
        "enum Failure:\n    Empty\n    Message(text: str)\n    Pair(first: i64, second: str)\n";
    for expression in [
        "Failure.Unknown",
        "Failure.Message",
        "Failure.Empty(1)",
        "Failure.Message(1)",
        "Failure.Pair(1)",
        "Failure.Pair(first=1, missing=\"x\")",
    ] {
        f.rejected(
            &format!(
                "{definition}def bad() -> Result[i64, Failure]:\n    return fail({expression})\n"
            ),
            "Failure",
        );
    }
}

#[test]
fn enum_matches_require_exhaustive_unique_matching_variants_and_payload_bindings() {
    let f = Fixture::new();
    let definitions = "enum Failure:\n    Empty\n    Message(text: str)\n    Pair(first: i64, second: str)\nenum Other:\n    Empty\n";
    for (cases, reason) in [
        ("        case Failure.Empty:\n            return 0\n", "網羅"),
        ("        case Failure.Empty:\n            return 0\n        case Failure.Empty:\n            return 1\n        case Failure.Message(text):\n            return 2\n        case Failure.Pair(first, second):\n            return 3\n", "重複"),
        ("        case Other.Empty:\n            return 0\n        case Failure.Message(text):\n            return 1\n        case Failure.Pair(first, second):\n            return 2\n", "型が一致"),
        ("        case Failure.Empty:\n            return 0\n        case Failure.Message:\n            return 1\n        case Failure.Pair(first, second):\n            return 2\n", "payload"),
        ("        case Failure.Empty:\n            return 0\n        case Failure.Message(text):\n            return 1\n        case Failure.Pair(value, value):\n            return 2\n", "重複"),
    ] {
        f.rejected(&format!("{definitions}def bad(error: Failure) -> i64:\n    match error:\n{cases}"), reason);
    }
}

#[test]
fn enum_payload_moves_and_borrowed_views_cannot_escape_match_arms() {
    let f = Fixture::new();
    let definition = "enum Failure:\n    Empty\n    Message(text: str)\n";
    f.rejected(&format!("{definition}def take(value: str):\n    print(value)\ndef bad(error: Failure):\n    match error:\n        case Failure.Empty:\n            print(0)\n        case Failure.Message(text):\n            take(text)\n            print(text)\n"), "text");
    f.rejected(&format!("{definition}def bad(error: Failure, fallback: view[str]) -> view[str]:\n    match error:\n        case Failure.Empty:\n            return fallback\n        case Failure.Message(text):\n            return view(text)\n"), "view");
    f.rejected(&format!("{definition}def consume(error: Failure):\n    match error:\n        case Failure.Empty:\n            print(0)\n        case Failure.Message(text):\n            print(text)\ndef bad(error: Failure):\n    consume(error)\n    consume(error)\n"), "error");
}

#[test]
fn same_named_module_enums_are_distinct_error_types_for_try_and_patterns() {
    let f = Fixture::new();
    for name in ["left.nagi", "right.nagi"] {
        f.write(name, "enum Failure:\n    Missing\n    Message(text: str)\ndef failed() -> Result[i64, Failure]:\n    return fail(Failure.Missing)\n");
    }
    for body in [
        "def bad() -> Result[i64, right.Failure]:\n    value = try left.failed()\n    return ok(value)\n",
        "def bad(error: left.Failure) -> i64:\n    match error:\n        case right.Failure.Missing:\n            return 0\n        case right.Failure.Message(text):\n            return 1\n",
    ] {
        f.rejected(&format!("import \"left.nagi\" as left\nimport \"right.nagi\" as right\n{body}"), "Failure");
    }
}

#[test]
fn direct_parser_builtin_enum_names_require_canonical_source_loading() {
    for name in ["Error", "i64", "bool", "Result"] {
        let mut direct = parser::parse(&format!("enum {name}:\n    Unit\n"), true).unwrap();
        let error = check::check(&mut direct)
            .expect_err("a raw builtin enum name cannot establish a distinct canonical identity");
        assert!(error.contains(name), "{error}");
        assert!(error.contains("source::load"), "{error}");
    }
    let mut normal = parser::parse("enum AuthError:\n    Missing\n", true).unwrap();
    check::check(&mut normal).unwrap();
}

#[test]
fn file_loaded_builtin_spelled_enums_keep_literal_types_and_variant_identity() {
    let f = Fixture::new();
    f.write(
        "main.nagi",
        r#"enum Error:
    Unit
enum i64:
    Unit
enum bool:
    Unit
enum Result:
    Unit
def main():
    first = Error.Unit
    second = i64.Unit
    third = bool.Unit
    fourth = Result.Unit
    number = 40
    flag = true
    assert_true(flag)
    assert_true(number + 2 == 42)
    match first:
        case Error.Unit:
            assert_true(true)
    match second:
        case i64.Unit:
            assert_true(true)
    match third:
        case bool.Unit:
            assert_true(true)
    match fourth:
        case Result.Unit:
            assert_true(true)
    print("builtin spelled enum identity preserved")
"#,
    );
    let saved = f.roundtrip("main.nagi");
    let main = saved
        .program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.body[4].binding_type,
        Some(nagic::ast::Type::named("i64"))
    );
    assert_eq!(
        main.body[5].binding_type,
        Some(nagic::ast::Type::named("bool"))
    );
    f.run_high_and_saved_low("builtin spelled enum identity preserved");
}
