use nagic::{check, emit, parser, source};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi-view-parameter-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn compile_and_run_drop_order(&self, name: &str, program: &nagic::ast::Program) {
        let adapter = self.0.join(format!("{name}-native.rs"));
        fs::write(&adapter, DROP_ADAPTER).unwrap();
        let rust = format!(
            "{}\n#[path = {}]\nmod native;\n{}",
            emit::rust(program).unwrap(),
            serde_json::to_string(&adapter.to_string_lossy()).unwrap(),
            DROP_ORDER_ASSERTIONS
        );
        let file = self.0.join(format!("{name}.rs"));
        let binary = self
            .0
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        fs::write(&file, rust).unwrap();
        let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2021", "--test"])
            .arg(&file)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(&binary).output().unwrap();
        assert!(
            output.status.success(),
            "{name}: {}{}",
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

fn checked(text: &str, high: bool) -> nagic::ast::Program {
    let mut program = parser::parse(text, high).unwrap_or_else(|error| panic!("{text}\n{error}"));
    check::check(&mut program).unwrap_or_else(|error| panic!("{text}\n{error}"));
    program
}

#[test]
fn local_parameter_updates_compile_and_run_in_high_and_saved_low() {
    let high = checked(LOCAL_UPDATES, true);
    let low = checked(&emit::low(&high), false);
    let fixture = Fixture::new();
    for (name, program) in [("high", high), ("low", low)] {
        let file = fixture.0.join(format!("{name}.rs"));
        let binary = fixture
            .0
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        fs::write(&file, emit::rust(&program).unwrap() + ASSERTIONS).unwrap();
        let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2021", "--test"])
            .arg(&file)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(&binary).output().unwrap();
        assert!(
            output.status.success(),
            "{name}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn views_of_owned_record_fields_can_be_used_locally_through_parameters() {
    let fixture = Fixture::new();
    let source = "class Holder:\n    text: str\ndef direct(part: view[str], holder: Holder) -> str:\n    part = view(holder.text)\n    return copy(part)\ndef appended(parts: List[view[str]], holder: Holder) -> str:\n    append(parts, view(holder.text))\n    return copy(parts[1])\ndef main():\n    original = \"original\"\n    print(direct(view(original), Holder(text=\"field\")))\n    print(appended([view(original)], Holder(text=\"appended field\")))\n";
    fs::write(fixture.0.join("main.nagi"), source).unwrap();
    let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
    check::check(&mut loaded.program).unwrap();
    let low = emit::low(&loaded.program);
    checked(&low, false);
    fs::write(fixture.0.join("main.low"), low).unwrap();
    let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("native-target")
        });
    for entry in ["main.nagi", "main.low"] {
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(&fixture.0)
            .args(["run", entry, "--no-project", "--out", "build"])
            .env("CARGO_NET_OFFLINE", "true")
            .env("NAGI_NATIVE_TARGET_DIR", &target)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{entry}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout)
                .unwrap()
                .replace("\r\n", "\n")
                .trim(),
            "field\nappended field"
        );
    }
}

#[test]
fn local_borrows_still_cannot_escape_through_reassigned_parameters() {
    for (high, low) in [
        ("def bad(part: view[str]) -> view[str]:\n    local = \"inner\"\n    part = view(local)\n    return part\n", "fn bad(part: view[str]) -> view[str] { let local: str = \"inner\"; part = view(local); return part; }\n"),
        ("def bad(parts: List[view[str]]) -> List[view[str]]:\n    local = \"inner\"\n    append(parts, view(local))\n    return parts\n", "fn bad(parts: List[view[str]]) -> List[view[str]] { let local: str = \"inner\"; append(parts, view(local)); return parts; }\n"),
        ("async def bad(part: view[str]) -> view[str]:\n    local = \"inner\"\n    part = view(local)\n    return part\n", "async fn bad(part: view[str]) -> view[str] { let local: str = \"inner\"; part = view(local); return part; }\n"),
    ] {
        for (text, is_high) in [(high, true), (low, false)] {
            let mut program = parser::parse(text, is_high).unwrap();
            let error = check::check(&mut program).expect_err(text);
            assert!(error.contains("view escapes"), "{text}\n{error}");
        }
    }
}

#[test]
fn rebinding_is_synthetic_and_does_not_change_signatures_or_extern_wrappers() {
    let program = checked("def read(part: view[str], callback: fn[i64, i64], value: i64) -> i64:\n    print(part)\n    return callback(value)\n", true);
    let generated = emit::rust_with_lines(&program).unwrap();
    assert!(generated
        .text
        .contains("pub fn read<'a>(mut part: &'a ::std::primitive::str"));
    let mut prologues = 0;
    for (index, line) in generated.text.lines().enumerate() {
        if line.trim_start().starts_with("let mut part:") {
            assert_eq!(line.trim(), "let mut part: &::std::primitive::str = part;");
            assert_eq!(generated.line_origin(index + 1), None);
            prologues += 1;
        }
        if line.contains("println!") {
            assert_eq!(generated.line_origin(index + 1), Some(2));
        }
        if line.contains("return callback(value)") {
            assert_eq!(generated.line_origin(index + 1), Some(3));
        }
    }
    assert_eq!(prologues, 1);
    assert!(!generated.text.contains("let mut callback:"));
    assert!(!generated.text.contains("let mut value:"));
    let external = checked(
        "@rust(\"native::read\")\nextern def read(part: view[str]) -> i64\n",
        true,
    );
    let external = emit::rust(&external).unwrap();
    assert!(external.contains("native::read(part)"));
    assert!(!external.contains("let mut part:"));
}

#[test]
fn owning_parameters_keep_their_drop_order_when_a_view_container_is_rebound() {
    let high = checked(DROP_ORDER_HIGH, true);
    let saved_low = checked(&emit::low(&high), false);
    let handwritten_low = checked(DROP_ORDER_LOW, false);
    let fixture = Fixture::new();

    for (name, program) in [
        ("high", &high),
        ("saved-low", &saved_low),
        ("handwritten-low", &handwritten_low),
    ] {
        let generated = emit::rust_with_lines(program).unwrap();
        assert!(generated.text.contains(
            "pub fn observe<'a>(mut before: Guard, mut first: ::std::result::Result<&'a ::std::primitive::str, Guard>, mut second: Guard, mut after: Guard) -> () {"
        ));
        let mut prologues = 0;
        for (index, line) in generated.text.lines().enumerate() {
            let line = line.trim_start();
            if line.starts_with("let mut before:")
                || line.starts_with("let mut first:")
                || line.starts_with("let mut second:")
                || line.starts_with("let mut after:")
            {
                assert_eq!(generated.line_origin(index + 1), None);
                prologues += 1;
            }
        }
        assert_eq!(prologues, 8);
        fixture.compile_and_run_drop_order(name, program);
    }
}

const LOCAL_UPDATES: &str = r#"def direct(part: view[str]) -> str:
    local = "direct"
    part = view(local)
    return copy(part)
def branch(part: view[str], flag: bool) -> str:
    if flag:
        local = "branch"
        part = view(local)
        return copy(part)
    return copy(part)
def looped(part: view[str]) -> str:
    for number in range(1):
        local = "loop"
        part = view(local)
        return copy(part)
    return copy(part)
def appended(parts: List[view[str]]) -> str:
    local = "appended"
    append(parts, view(local))
    return copy(parts[len(parts) - 1])
def nested(parts: List[List[view[str]]]) -> i64:
    local = "nested"
    append(parts, [view(local)])
    return len(parts)
def nullable(part: Option[view[str]]) -> str:
    local = "nullable"
    part = some(view(local))
    match part:
        case Some(value):
            return copy(value)
        case None:
            return "absent"
def nested_slice(parts: view[view[str]]) -> i64:
    local = "nested slice"
    replacement = [view(local), view(local)]
    parts = view(replacement)
    return len(parts)
def identity(part: view[str]) -> view[str]:
    return part
def invoke(callback: fn[view[str], view[str]], part: view[str]) -> view[str]:
    return callback(part)
async def ready():
    return
async def after_await(part: view[str]) -> str:
    local = "async"
    part = view(local)
    await ready()
    return copy(part)
"#;

const ASSERTIONS: &str = r#"
#[test]
fn values_and_input_bound_returns() {
    let original = String::from("original");
    assert_eq!(direct(&original), "direct");
    assert_eq!(branch(&original, true), "branch");
    assert_eq!(branch(&original, false), "original");
    assert_eq!(looped(&original), "loop");
    assert_eq!(appended(vec![&original]), "appended");
    assert_eq!(nested(vec![vec![&original]]), 2);
    assert_eq!(nullable(Some(&original)), "nullable");
    assert_eq!(nested_slice(&[&original]), 2);
    assert_eq!(identity(&original), "original");
    assert_eq!(invoke(identity, &original), "original");
    let mut future = std::pin::pin!(after_await(&original));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    match std::future::Future::poll(future.as_mut(), &mut context) {
        std::task::Poll::Ready(value) => assert_eq!(value, "async"),
        std::task::Poll::Pending => panic!("the fixture only awaits an immediately ready function"),
    }
}
"#;

const DROP_ORDER_HIGH: &str = r#"enum Tag:
    Text(value: str)

class Guard:
    id: i64
    tag: Tag

def observe(before: Guard, first: Result[view[str], Guard], second: Guard, after: Guard):
    return

def observe_panicking(before: Guard, first: Result[view[str], Guard], second: Guard, after: Guard):
    assert_true(False)
"#;

const DROP_ORDER_LOW: &str = r#"enum Tag { Text(value: str) }
record Guard { id: i64; tag: Tag }
fn observe(before: Guard, first: Result[view[str], Guard], second: Guard, after: Guard) { return; }
fn observe_panicking(before: Guard, first: Result[view[str], Guard], second: Guard, after: Guard) { assert_true(False); }
"#;

const DROP_ADAPTER: &str = r#"use std::cell::RefCell;

thread_local! {
    static DROPPED: RefCell<Vec<i64>> = const { RefCell::new(Vec::new()) };
}

pub fn take_drops() -> Vec<i64> {
    DROPPED.with(|dropped| ::std::mem::take(&mut *dropped.borrow_mut()))
}

impl Drop for super::Guard {
    fn drop(&mut self) {
        DROPPED.with(|dropped| dropped.borrow_mut().push(self.id));
    }
}
"#;

const DROP_ORDER_ASSERTIONS: &str = r#"
fn guard(id: i64) -> Guard {
    Guard {
        id,
        tag: Tag::Text {
            value: format!("owned-{id}"),
        },
    }
}

#[test]
fn normal_return_keeps_reverse_parameter_drop_order() {
    observe(guard(1), Err(guard(2)), guard(3), guard(4));
    assert_eq!(native::take_drops(), vec![4, 3, 2, 1]);
}

#[test]
fn panic_unwind_keeps_reverse_parameter_drop_order() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        observe_panicking(guard(1), Err(guard(2)), guard(3), guard(4));
    }));
    assert!(result.is_err());
    assert_eq!(native::take_drops(), vec![4, 3, 2, 1]);
}
"#;
