use nagic::{check, emit, modules, parser, source, stdlib, symbols};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

static ID: AtomicU64 = AtomicU64::new(0);
static NATIVE_RUN: Mutex<()> = Mutex::new(());

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi-result-stdlib-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }

    fn checked(&self, name: &str) -> source::Sources {
        let mut loaded = source::load(&self.0.join(name), name.ends_with(".nagi")).unwrap();
        check::check(&mut loaded.program)
            .unwrap_or_else(|error| panic!("{}", loaded.diagnostic(&error)));
        loaded
    }

    fn roundtrip(&self) -> source::Sources {
        let loaded = self.checked("main.nagi");
        let low = emit::low(&loaded.program);
        let mut independent = parser::parse(&low, false).unwrap();
        check::check(&mut independent).unwrap_or_else(|error| panic!("{error}\n{low}"));
        modules::validate(&independent).unwrap();
        let identities = |program: &nagic::ast::Program| {
            program
                .modules
                .definitions
                .iter()
                .map(|definition| (definition.id.clone(), definition.symbol.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(identities(&independent), identities(&loaded.program));
        self.write("saved.low", &low);
        self.checked("saved.low")
    }

    fn rejected(&self, source: &str, expected: &str) {
        self.write("main.nagi", source);
        let unchecked = source::load(&self.0.join("main.nagi"), true).unwrap();
        self.write("saved.low", &emit::low(&unchecked.program));
        for name in ["main.nagi", "saved.low"] {
            let mut loaded = source::load(&self.0.join(name), name.ends_with(".nagi")).unwrap();
            let error = check::check(&mut loaded.program).expect_err("invalid map_error accepted");
            assert!(error.contains(expected), "{name}: {error}\n{source}");
            // These failures must not depend on Cargo or installed runtime files.
            let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
                .current_dir(&self.0)
                .args(["build", name, "--no-project", "--out", "rejected"])
                .env("PATH", "")
                .env("NAGI_ROOT", self.0.join("missing-runtime"))
                .output()
                .unwrap();
            assert!(!output.status.success());
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains(expected), "{name}: {stderr}");
            assert!(!stderr.contains("Rust backend rejected"), "{stderr}");
            assert!(!self.0.join("rejected/src/main.rs").exists());
        }
    }

    fn run_both(&self, expected: &str) {
        self.run_both_with_args(expected, &[]);
    }

    fn run_both_with_args(&self, expected: &str, extra: &[&str]) {
        let _guard = NATIVE_RUN.lock().unwrap();
        let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join("native-target")
            });
        for name in ["main.nagi", "saved.low"] {
            let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
                .current_dir(&self.0)
                .args(["run", name, "--no-project", "--out", "build"])
                .args(extra)
                .env("NAGI_NATIVE_TARGET_DIR", &target)
                .env("CARGO_NET_OFFLINE", "true")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stderr = String::from_utf8(output.stderr).unwrap();
            let launcher = stderr
                .lines()
                .find(|line| line.starts_with("native: "))
                .unwrap();
            let actual = String::from_utf8(output.stdout).unwrap();
            assert!(Path::new(
                launcher
                    .trim_end_matches('\r')
                    .strip_prefix("native: ")
                    .unwrap()
            )
            .is_file());
            assert_eq!(actual.trim(), expected, "{name}");
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const PREFIX: &str = "import std.result as result\nenum Problem:\n    Invalid(cause: Error)\ndef convert(cause: Error) -> Problem:\n    return Problem.Invalid(cause)\n";

#[test]
fn inferred_mapping_keeps_enum_class_aliases_and_borrowed_success_through_saved_low() {
    let fixture = Fixture::new();
    fixture.write("main.nagi", &format!("{PREFIX}from std.result import map_error as convert_result\nclass Wrapped:\n    cause: Error\ndef wrap(cause: Error) -> Wrapped:\n    return Wrapped(cause=cause)\ndef parse(text: view[str]) -> Result[i64, Problem]:\n    mapper = convert\n    return result.map_error(parse_i64(text), mapper)\ndef propagate(text: view[str]) -> Result[i64, Problem]:\n    number = try parse(text)\n    return ok(number + 1)\ndef wrap_parse(text: view[str]) -> Result[i64, Wrapped]:\n    return convert_result(parse_i64(text), wrap)\ndef keep(value: view[str]) -> Result[view[str], Problem]:\n    pending: Result[view[str], Error] = ok(value)\n    return result.map_error(pending, convert)\ndef owned(value: Result[str, Error]) -> Result[str, Problem]:\n    return result.map_error(value, convert)\n"));
    let saved = fixture.roundtrip();
    assert!(saved
        .program
        .modules
        .definitions
        .iter()
        .any(|definition| definition.id == stdlib::function_id(stdlib::Operation::ResultMapError)));
    let rust = emit::rust(&saved.program).unwrap();
    assert!(
        rust.contains("::nagi_runtime::result::map_error("),
        "{rust}"
    );
    assert!(!rust.contains(".clone()"), "{rust}");
    assert!(!rust.contains("Box::new"), "{rust}");
    let index = symbols::index(&saved, &[&saved.program]).unwrap();
    let source = index["standard_sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["file"] == stdlib::RESULT_MODULE_ID)
        .unwrap();
    assert!(source["text"]
        .as_str()
        .unwrap()
        .contains("def map_error(value: Result[T, E], mapper: fn[E, F]) -> Result[T, F]"));
    let operation = index["standard_modules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == stdlib::RESULT_MODULE_ID)
        .unwrap();
    // Empty explicit type parameters are omitted by the symbol serializer;
    // map_error infers them from its arguments instead of exposing brackets.
    assert!(operation["members"][0].get("typeParameters").is_none());
    assert_eq!(operation["members"][0]["return_type"], "Result[T, F]");
}

#[test]
fn native_success_failure_and_original_cause_survive_high_and_saved_low() {
    let fixture = Fixture::new();
    fixture.write("main.nagi", &format!("{PREFIX}class Wrapped:\n    cause: Error\ndef mapped(cause: Error) -> Problem:\n    print(\"mapped\")\n    return Problem.Invalid(cause)\ndef wrap(cause: Error) -> Wrapped:\n    return Wrapped(cause=cause)\ndef read(text: view[str]) -> Result[i64, Problem]:\n    value = try result.map_error(parse_i64(text), mapped)\n    return ok(value + 1)\ndef keep(value: view[str]) -> Result[view[str], Problem]:\n    pending: Result[view[str], Error] = ok(value)\n    return result.map_error(pending, mapped)\ndef main():\n    match read(view(\"41\")):\n        case Ok(value):\n            print(value)\n        case Err(_):\n            assert_true(False)\n    match read(view(\"oops\")):\n        case Ok(_):\n            assert_true(False)\n        case Err(problem):\n            match problem:\n                case Problem.Invalid(cause):\n                    print(error_kind(cause))\n                    assert_true(len(view(error_message(cause))) > 0)\n    match result.map_error(parse_i64(\"bad\"), wrap):\n        case Ok(_):\n            assert_true(False)\n        case Err(problem):\n            print(error_kind(problem.cause))\n    owner = \"borrowed\"\n    match keep(view(owner)):\n        case Ok(value):\n            print(value)\n        case Err(_):\n            assert_true(False)\n"));
    fixture.roundtrip();
    fixture.run_both("42\nmapped\ninvalid\ninvalid\nborrowed");
}

#[test]
fn mapper_contract_errors_fail_before_rust_backend_in_high_and_saved_low() {
    let fixture = Fixture::new();
    for (body, expected) in [
        ("def bad() -> Result[i64, Problem]:\n    return result.map_error(42, convert)\n", "Result[T,E]"),
        ("def wrong(value: str) -> i64:\n    return len(view(value))\ndef bad() -> Result[i64, i64]:\n    return result.map_error(parse_i64(\"bad\"), wrong)\n", "型が一致しません"),
        ("def wrong(cause: Error, number: i64) -> Problem:\n    return Problem.Invalid(cause)\ndef bad() -> Result[i64, Problem]:\n    return result.map_error(parse_i64(\"bad\"), wrong)\n", "同期fn[E,F]"),
        ("async def wrong(cause: Error) -> Problem:\n    return Problem.Invalid(cause)\ndef bad() -> Result[i64, Problem]:\n    return result.map_error(parse_i64(\"bad\"), wrong)\n", "同期fn[E,F]"),
        ("def bad() -> Result[i64, Problem]:\n    return result.map_error[i64, Error, Problem](parse_i64(\"bad\"), convert)\n", "型引数は指定できません"),
        ("def bad() -> Result[i64, Problem]:\n    return result.map_error(parse_i64(\"bad\"))\n", "2個"),
        ("def identity(value: view[str]) -> view[str]:\n    return value\ndef bad(text: view[str]) -> Result[i64, view[str]]:\n    value: Result[i64, view[str]] = fail(text)\n    return result.map_error(value, identity)\n", "エラーにview"),
    ] {
        fixture.rejected(&format!("{PREFIX}{body}"), expected);
    }
}

#[test]
fn mapping_moves_input_left_to_right_and_preserves_borrow_origins() {
    let fixture = Fixture::new();
    for (body, expected) in [
        ("def bad() -> Result[i64, Error]:\n    value = parse_i64(\"bad\")\n    mapped = result.map_error(value, convert)\n    return value\n", "move"),
        ("def select(value: Result[i64, Error]) -> fn[Error, Problem]:\n    match value:\n        case Ok(_):\n            return convert\n        case Err(_):\n            return convert\ndef bad() -> Result[i64, Problem]:\n    value = parse_i64(\"bad\")\n    return result.map_error(value, select(value))\n", "move"),
        ("def select(text: str) -> fn[Error, Problem]:\n    print(text)\n    return convert\ndef bad() -> Result[view[str], Problem]:\n    owner = \"borrowed\"\n    value: Result[view[str], Error] = ok(view(owner))\n    return result.map_error(value, select(owner))\n", "参照されています"),
        ("def bad() -> Result[view[str], Problem]:\n    owner = \"borrowed\"\n    value: Result[view[str], Error] = ok(view(owner))\n    return result.map_error(value, convert)\n", "escapes its lifetime"),
        ("def bad() -> Result[view[str], Problem]:\n    value = result.map_error(ok(view(\"temporary\")), convert)\n    return value\n", "一時的"),
    ] {
        fixture.rejected(&format!("{PREFIX}{body}"), expected);
    }
}

#[test]
fn try_keeps_exact_error_identity_and_map_error_does_not_reserve_local_names() {
    let fixture = Fixture::new();
    fixture.write("other.nagi", "enum Problem:\n    Invalid(cause: Error)\ndef convert(cause: Error) -> Problem:\n    return Problem.Invalid(cause)\n");
    fixture.rejected(&format!("{PREFIX}import \"other.nagi\" as other\ndef bad() -> Result[i64, Problem]:\n    value = try result.map_error(parse_i64(\"bad\"), other.convert)\n    return ok(value)\n"), "型が一致しません");
    fixture.write("main.nagi", "import std.result as result\ndef map_error(value: i64) -> i64:\n    return value + 1\ndef plain(value: i64) -> i64:\n    return map_error(value)\ndef local(value: i64) -> i64:\n    map_error = plain\n    return map_error(value)\n");
    fixture.roundtrip();
}

#[test]
fn static_error_labels_avoid_read_allocations_but_keep_owned_results_and_shadowing() {
    let fixture = Fixture::new();
    fixture.write(
        "main.nagi",
        r#"@rust("native::probe")
extern def probe(reads: fn[Error, unit], copied: fn[Error, unit], owned: fn[Error, str])
@rust("native::consume")
extern def consume(label: str)
def read(label: view[str]):
    assert_true(len(label) == 7)
def reads(cause: Error):
    for number in range(128):
        assert_true(error_kind(cause) == "invalid")
        assert_true(error_kind(cause) != "missing")
        assert_true("invalid" == error_kind(cause))
        assert_true(len(error_kind(cause)) == 7)
        assert_true(len(view(error_kind(cause))) == 7)
        read(view(error_kind(cause)))
    print(error_kind(cause))
def copied(cause: Error):
    consume(copy(view(error_kind(cause))))
def owned(cause: Error) -> str:
    label = error_kind(cause)
    consume(copy(view(label)))
    return label
def main():
    probe(reads, copied, owned)
"#,
    );
    fixture.write(
        "native.rs",
        r#"use nagi_runtime::{Error, metrics::measure};
pub fn probe(reads: fn(Error), copied: fn(Error), owned: fn(Error) -> String) {
    println!("warm");
    let cause = Error::invalid("test");
    let (_, allocations) = measure(|| reads(cause));
    println!("{}", allocations.allocations);
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
    let cause = Error::invalid("test");
    let (_, allocations) = measure(|| copied(cause));
    println!("{}", allocations.allocations);
    assert_eq!(allocations.allocations, 1);
    assert_eq!(allocations.reallocations, 0);
    consume(owned(Error::invalid("test")));
}
#[inline(never)]
pub fn consume(label: String) {
    assert_eq!(std::hint::black_box(label).as_str(), "invalid");
}
"#,
    );
    fixture.roundtrip();
    fixture.run_both_with_args("warm\ninvalid\n0\n1", &["--rust", "native.rs"]);

    fixture.write("main.nagi", "def error_kind(value: i64) -> str:\n    return \"custom\"\ndef main():\n    assert_true(error_kind(1) == \"custom\")\n    assert_true(len(error_kind(1)) == 6)\n    print(error_kind(1))\n");
    fixture.roundtrip();
    fixture.run_both("custom");
}
