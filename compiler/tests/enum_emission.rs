#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, parser, source};
use std::{fs, path::PathBuf, process::Command};

static FIXTURE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi-enum-emission-{}-{}",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }

    fn compile_and_run(&self, text: &str) {
        let source = self.0.join("generated.rs");
        let binary = self
            .0
            .join(format!("generated{}", std::env::consts::EXE_SUFFIX));
        fs::write(&source, text).unwrap();
        let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2021", "--test", "-D", "unused-imports"])
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{text}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(binary).output().unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn checked_low(high: &str) -> nagic::ast::Program {
    let mut parsed = parser::parse(high, true).unwrap();
    check::check(&mut parsed).unwrap_or_else(|error| panic!("{error}\n{high}"));
    let emitted = emit::low_with_lines(&parsed);
    let mut low = parser::parse(&emitted.text, false).unwrap();
    emitted.restore_lines(&mut low).unwrap();
    check::check(&mut low).unwrap_or_else(|error| panic!("{error}\n{}", emitted.text));
    low
}

const ALLOCATION_TRACKER: &str = r#"
struct AllocationTracker;
thread_local! {
    static ALLOCATIONS: ::std::cell::Cell<::std::option::Option<::std::primitive::usize>> = const { ::std::cell::Cell::new(None) };
}
fn count_allocation() {
    let _ = ALLOCATIONS.try_with(|count| {
        if let Some(value) = count.get() { count.set(Some(value + 1)); }
    });
}
unsafe impl ::std::alloc::GlobalAlloc for AllocationTracker {
    unsafe fn alloc(&self, layout: ::std::alloc::Layout) -> *mut u8 {
        count_allocation();
        ::std::alloc::GlobalAlloc::alloc(&::std::alloc::System, layout)
    }
    unsafe fn alloc_zeroed(&self, layout: ::std::alloc::Layout) -> *mut u8 {
        count_allocation();
        ::std::alloc::GlobalAlloc::alloc_zeroed(&::std::alloc::System, layout)
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: ::std::alloc::Layout, size: usize) -> *mut u8 {
        count_allocation();
        ::std::alloc::GlobalAlloc::realloc(&::std::alloc::System, pointer, layout, size)
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: ::std::alloc::Layout) {
        ::std::alloc::GlobalAlloc::dealloc(&::std::alloc::System, pointer, layout);
    }
}
#[global_allocator] static TRACKER: AllocationTracker = AllocationTracker;
fn allocations(action: impl FnOnce()) -> usize {
    ALLOCATIONS.with(|count| count.set(Some(0)));
    action();
    ALLOCATIONS.with(|count| count.replace(None).unwrap())
}
"#;

#[test]
fn native_enum_constructors_patterns_and_result_propagation_roundtrip() {
    let fixture = Fixture::new();
    let program = checked_low(
        "enum Failure:\n    Empty\n    Invalid(message: str)\n    Number(value: i64, enabled: bool)\ndef success() -> Result[i64, Failure]:\n    return ok(42)\ndef number(value: i64) -> Result[i64, Failure]:\n    return fail(Failure.Number(enabled=True, value=value))\ndef forward(value: i64) -> Result[i64, Failure]:\n    return ok(try number(value))\ndef describe(error: Failure) -> i64:\n    match error:\n        case Failure.Empty:\n            return 0\n        case Failure.Invalid(message):\n            return len(view(message))\n        case Failure.Number(value, _):\n            return value\ndef payload() -> Failure:\n    return Failure.Invalid(\"bad\")\ndef empty() -> Failure:\n    return Failure.Empty\n",
    );
    let mut rust = emit::rust(&checked_emission::seal(&program)).unwrap();
    assert!(rust.contains("pub enum Failure"));
    assert!(rust.contains("::std::result::Result::Err(Failure::Number"));
    assert!(!rust.contains("Box::"));
    assert!(!rust.contains(".clone()"));
    assert!(!rust.contains("serde"));
    rust.push_str(ALLOCATION_TRACKER);
    rust.push_str(
        "\n#[test] fn native_enum_values() {\n    assert_eq!(allocations(|| { assert_eq!(success().unwrap(), 42); }), 0);\n    assert_eq!(allocations(|| { assert_eq!(describe(forward(7).unwrap_err()), 7); }), 0);\n    assert_eq!(describe(payload()), 3);\n    assert_eq!(allocations(|| { assert_eq!(describe(empty()), 0); }), 0);\n    let value = Failure::Number { value: 9, enabled: true };\n    assert_eq!(describe(value), 9);\n}\n",
    );
    fixture.compile_and_run(&rust);
}

#[test]
fn module_enum_aliases_preserve_identity_and_escape_variant_fields() {
    let fixture = Fixture::new();
    fixture.write(
        "errors.nagi",
        "enum Choice:\n    Empty\n    type(self: i64)\n    Message(text: str)\n",
    );
    fixture.write(
        "main.nagi",
        "import \"errors.nagi\" as Option\nfrom \"errors.nagi\" import Choice as SavedChoice\ndef make(value: i64) -> SavedChoice:\n    return Option.Choice.type(value)\ndef describe(error: SavedChoice) -> i64:\n    match error:\n        case SavedChoice.Empty:\n            return 0\n        case SavedChoice.type(type):\n            return type\n        case SavedChoice.Message(message):\n            return len(view(message))\n",
    );
    let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
    check::check(&mut loaded.program)
        .unwrap_or_else(|error| panic!("{}", loaded.diagnostic(&error)));
    let identity = loaded
        .program
        .modules
        .resolve_root_path("SavedChoice")
        .unwrap()
        .id
        .clone();
    let emitted = emit::low(&loaded.program);
    let mut low = parser::parse(&emitted, false).unwrap();
    check::check(&mut low).unwrap();
    assert_eq!(
        low.modules.resolve_root_path("SavedChoice").unwrap().id,
        identity
    );
    fixture.write("saved.low", &emitted);
    let mut saved = source::load(&fixture.0.join("saved.low"), false).unwrap();
    check::check(&mut saved.program).unwrap();
    assert_eq!(
        saved
            .program
            .modules
            .resolve_root_path("SavedChoice")
            .unwrap()
            .id,
        identity
    );
    let mut rust = emit::rust(&checked_emission::seal(&saved.program)).unwrap();
    assert!(rust.contains("r#type"));
    rust.push_str(
        "\n#[test] fn aliases_name_one_native_enum() {\n    let value: Option::Choice = make(7);\n    let same: SavedChoice = value;\n    assert_eq!(format!(\"{:?}\", &same), \"type { self: 7 }\");\n    assert_eq!(describe(same), 7);\n    assert_eq!(describe(Option::Choice::Message { text: String::from(\"abc\") }), 3);\n    assert_eq!(describe(SavedChoice::Empty), 0);\n}\n",
    );
    fixture.compile_and_run(&rust);
}

#[test]
fn error_cause_records_move_payloads_without_serde_or_clone_requirements() {
    let fixture = Fixture::new();
    fixture.write("main.nagi",
        "class Failure:\n    type: Error\nclass Wrapper:\n    failure: Failure\ndef make(cause: Error) -> Result[i64, Wrapper]:\n    return fail(Wrapper(failure=Failure(type=cause)))\n",
    );
    let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
    check::check(&mut loaded.program).unwrap();
    let mut program = parser::parse(&emit::low(&loaded.program), false).unwrap();
    check::check(&mut program).unwrap();
    let mut rust = emit::rust(&checked_emission::seal(&program)).unwrap();
    assert!(!rust.contains("serde"));
    assert!(!rust.contains("Clone"));
    assert!(!rust.contains(".clone()"));
    rust.push_str(
        "\nextern crate self as nagi_runtime;\n#[derive(Debug)] pub struct Error { pub message: String }\n#[test] fn cause_keeps_owned_message() {\n    let message = String::from(\"private cause\");\n    let address = message.as_ptr();\n    let error = make(Error { message }).unwrap_err();\n    assert_eq!(error.failure.r#type.message, \"private cause\");\n    assert_eq!(error.failure.r#type.message.as_ptr(), address);\n    assert_eq!(format!(\"{:?}\", error), \"Wrapper { failure: Failure { type: Error { message: \\\"private cause\\\" } } }\");\n}\n",
    );
    fixture.compile_and_run(&rust);
}

#[test]
fn data_records_keep_serde_while_enum_fields_remain_private() {
    let program = checked_low(
        "enum Failure:\n    Empty\nclass Data:\n    type: i64\nclass Private:\n    failure: Failure\n",
    );
    let rust = emit::rust(&checked_emission::seal(&program)).unwrap();
    let data = rust.split("pub struct Data").next().unwrap();
    assert!(data.contains("serde::Serialize"));
    assert!(rust.contains("#[serde(rename = \"type\")]"));
    let private = rust
        .split("pub struct Data")
        .nth(1)
        .unwrap()
        .split("pub struct Private")
        .next()
        .unwrap();
    assert!(!private.contains("serde::Serialize"));
    assert!(!rust.contains("FromRow for Failure"));
}

#[test]
fn custom_main_errors_need_only_debug_and_copy_enums_reuse_values() {
    let fixture = Fixture::new();
    let program = checked_low(
        "enum Code:\n    Ready\n    Number(value: i64)\ndef value(code: Code) -> i64:\n    match code:\n        case Code.Ready:\n            return 0\n        case Code.Number(number):\n            return number\ndef twice(code: Code) -> i64:\n    return value(code) + value(code)\ndef main() -> Result[unit, Code]:\n    return fail(Code.Ready)\n",
    );
    let mut rust = emit::rust(&checked_emission::seal(&program)).unwrap();
    assert!(rust.contains("#[derive(Debug, Clone, Copy)]"));
    assert!(rust.contains("eprintln!(\"{:?}\",e)"));
    rust.push_str(
        "\n#[test] fn copy_values() { assert_eq!(twice(Code::Number { value: 21 }), 42); }\n",
    );
    fixture.compile_and_run(&rust);
}
