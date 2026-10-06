// 承認済みの狭い所有代入移行。型・move・origin拒否と実生成Rustを別に観測する。
#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, parser, source};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "nagi-explicit-moves-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("exclusive move fixture: {error}"),
            }
        }
    }
    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }
    fn checked(&self, name: &str) -> Result<nagic::ast::Program, String> {
        let mut loaded = source::load(&self.0.join(name), name.ends_with(".nagi"))?;
        check::check(&mut loaded.program)?;
        Ok(loaded.program)
    }
    fn reject(&self, high: &str, low: &str, line: usize, reason: &str) {
        for (name, text) in [("bad.nagi", high), ("bad.low", low)] {
            self.write(name, text);
            let error = self.checked(name).expect_err(text);
            assert!(
                error.starts_with(&format!("line {line}:")),
                "{name}: {error}\n{text}"
            );
            assert!(error.contains(reason), "{name}: {error}\n{text}");
        }
    }
    fn run_three(&self, case: &str, high: &str, low: &str, adapter: &str, assertions: &str) {
        self.write("main.nagi", high);
        let program = self
            .checked("main.nagi")
            .expect("High check before backend");
        self.write("saved.low", &emit::low(&program));
        self.write("handwritten.low", low);
        let saved = self
            .checked("saved.low")
            .expect("independent saved Low check");
        let handwritten = self
            .checked("handwritten.low")
            .expect("handwritten Low check");
        // 保存Lowの再読込は元High fileへ依存してはならない。
        fs::remove_file(self.0.join("main.nagi")).unwrap();
        let independent = self
            .checked("saved.low")
            .expect("saved Low after High removal");
        assert_eq!(emit::low(&saved), emit::low(&independent));
        for (name, program) in [
            ("high", program),
            ("saved-low", independent),
            ("handwritten-low", handwritten),
        ] {
            let generated =
                emit::rust(&checked_emission::seal(&program)).expect("sealed Rust emission");
            let rust = self.0.join(format!("{name}.rs"));
            let binary = self
                .0
                .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
            fs::write(&rust, format!("{generated}\n{adapter}\n{assertions}")).unwrap();
            let compiled =
                Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
                    .args([
                        "--edition=2021",
                        "--test",
                        "-C",
                        "panic=unwind",
                        "-C",
                        "debuginfo=0",
                    ])
                    .arg(&rust)
                    .arg("-o")
                    .arg(&binary)
                    .output()
                    .unwrap();
            assert!(
                compiled.status.success(),
                "{name} backend build:\n{}\n{generated}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            let ran = Command::new(&binary).arg("--nocapture").output().unwrap();
            assert!(
                ran.status.success(),
                "{name} backend run:\n{}{}",
                String::from_utf8_lossy(&ran.stdout),
                String::from_utf8_lossy(&ran.stderr)
            );
            if let Some(directory) = std::env::var_os("NAGI_TEST_ARTIFACT_DIR") {
                let directory = PathBuf::from(directory);
                fs::create_dir_all(&directory).unwrap();
                fs::copy(
                    &rust,
                    directory.join(format!("explicit-move-{case}-{name}.rs")),
                )
                .unwrap();
                fs::write(
                    directory.join(format!("explicit-move-{case}-{name}.stdout")),
                    &ran.stdout,
                )
                .unwrap();
            }
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn implicit_noncopy_local_assignments_fail_at_the_original_source_line() {
    // importなし: 旧checkerはこれを受理する。未実装operationの拒否と区別するRED。
    let fixture = Fixture::new();
    for ty in [
        "str",
        "List[str]",
        "shared[str]",
        "owned[str]",
        "str?",
        "Result[i64, i64]",
    ] {
        fixture.reject(
            &format!("def transfer(value: {ty}):\n    replacement = value\n"),
            &format!(
                "fn transfer(value: {ty}) -> unit {{\n    let replacement: {ty} = value;\n}}\n"
            ),
            2,
            "非Copyの既存値 value の代入は暗黙moveできません。std.ownership.moveで明示してください",
        );
    }
    for (high, low, line) in [
        ("def transfer(value: str):\n    replacement = ((value))\n", "fn transfer(value: str) -> unit {\n    let replacement: str = ((value));\n}\n", 2),
        ("def transfer(value: str):\n    replacement: str = value\n", "fn transfer(value: str) -> unit {\n    let replacement: str = value;\n}\n", 2),
        ("def transfer(value: str):\n    replacement = \"old\"\n    replacement = value\n", "fn transfer(value: str) -> unit {\n    let replacement: str = \"old\";\n    replacement = value;\n}\n", 3),
        ("def transfer(value: str):\n    if True:\n        replacement = value\n", "fn transfer(value: str) -> unit {\n    if true {\n        let replacement: str = value;\n    }\n}\n", 3),
        ("def transfer(value: str):\n    for number in range(2):\n        replacement = value\n", "fn transfer(value: str) -> unit {\n    for number in range(2) {\n        let replacement: str = value;\n    }\n}\n", 3),
    ] { fixture.reject(high, low, line, "std.ownership.moveで明示してください"); }
}

#[test]
fn copy_inputs_keep_ordinary_assignment_and_move_expected_type_inference() {
    let fixture = Fixture::new();
    for ty in [
        "i8",
        "i16",
        "i32",
        "i64",
        "u8",
        "u16",
        "u32",
        "u64",
        "f32",
        "f64",
        "bool",
        "unit",
        "UUID",
        "timestamp",
        "owned[i64]",
        "i64?",
        "view[str]",
        "fn[i64, i64]",
    ] {
        fixture.write("copy.nagi", &format!("from std.ownership import move\ndef keep(value: {ty}) -> {ty}:\n    original = value\n    transferred = move(value)\n    return value\n"));
        let high = fixture
            .checked("copy.nagi")
            .unwrap_or_else(|error| panic!("{ty}: {error}"));
        fixture.write("copy.low", &emit::low(&high));
        fixture
            .checked("copy.low")
            .unwrap_or_else(|error| panic!("{ty}: {error}"));
    }
    fixture.write("hints.nagi", "from std.ownership import move\ndef hints():\n    small: i8 = move(1)\n    absent: i64? = move(None)\n    fresh = move(\"Nagi\")\n");
    let high = fixture.checked("hints.nagi").unwrap();
    fixture.write("hints.low", &emit::low(&high));
    fixture.checked("hints.low").unwrap();
}

#[test]
fn canonical_aliases_shadowing_and_transparent_values_run_in_all_three_sources() {
    Fixture::new().run_three("normal", NORMAL_HIGH, NORMAL_LOW, "", NORMAL_ASSERTIONS);
}

#[test]
fn move_operation_rejects_arity_type_arguments_and_future_transfer() {
    let fixture = Fixture::new();
    for call in ["move()", "move(1, 2)"] {
        fixture.reject(&format!("from std.ownership import move\ndef bad():\n    value = {call}\n"), &format!("from std.ownership import move;\nfn bad() -> unit {{\n    let value = {call};\n}}\n"), 3, "引数");
    }
    fixture.reject("from std.ownership import move\ndef bad():\n    value = move[i64](1)\n", "from std.ownership import move;\nfn bad() -> unit {\n    let value: i64 = move[i64](1);\n}\n", 3, "型引数");
    for (name, text) in [
        ("bad.nagi", "from std.ownership import move\nasync def child(value: view[str]):\n    print(value)\nasync def main() -> Result[unit, Error]:\n    owner = \"Nagi\"\n    async with scope:\n        spawn move(child(view(owner)))\n    return ok(print(0))\n"),
        ("bad.low", "from std.ownership import move;\nasync fn child(value: view[str]) -> unit { print(value); }\nasync fn main() -> Result[unit, Error] {\n    let owner: str = \"Nagi\";\n    scope {\n        spawn move(child(view(owner)));\n    }\n    return ok(print(0));\n}\n"),
    ] {
        fixture.write(name, text);
        let error = fixture.checked(name).expect_err("move must not hide a borrowed Future from spawn");
        let line = if name.ends_with("nagi") { 7 } else { 6 };
        assert!(error.starts_with(&format!("line {line}:")), "{error}");
        assert!(error.contains("Future") || error.contains("async呼び出し"), "{error}");
    }
}

#[test]
fn explicit_move_preserves_use_after_move_borrow_partial_move_and_loop_checks() {
    let fixture = Fixture::new();
    for (high, low, line, reason) in [
        ("from std.ownership import move\ndef bad(value: str):\n    moved = move(value)\n    print(value)\n", "from std.ownership import move;\nfn bad(value: str) -> unit {\n    let moved: str = move(value);\n    print(value);\n}\n", 4, "move後"),
        ("from std.ownership import move\ndef bad(value: str):\n    first = move(value)\n    second = move(value)\n", "from std.ownership import move;\nfn bad(value: str) -> unit {\n    let first: str = move(value);\n    let second: str = move(value);\n}\n", 4, "move後"),
        ("from std.ownership import move\ndef bad(value: str):\n    part = view(value)\n    moved = move(value)\n    print(part)\n", "from std.ownership import move;\nfn bad(value: str) -> unit {\n    let part: view[str] = view(value);\n    let moved: str = move(value);\n    print(part);\n}\n", 4, "参照"),
        ("from std.ownership import move\nclass Packet:\n    text: str\ndef bad(value: shared[Packet]) -> str:\n    return move(value.text)\n", "from std.ownership import move;\nrecord Packet { text: str; }\nfn bad(value: shared[Packet]) -> str {\n    return move(value.text);\n}\n", 5, "shared"),
        ("from std.ownership import move\nclass Packet:\n    text: str\ndef bad(value: Packet):\n    field = value.text\n    entire = move(value)\n", "from std.ownership import move;\nrecord Packet { text: str; }\nfn bad(value: Packet) -> unit {\n    let field: str = value.text;\n    let entire: Packet = move(value);\n}\n", 6, "move後"),
        ("from std.ownership import move\ndef bad(value: str):\n    if True:\n        moved = move(value)\n    print(value)\n", "from std.ownership import move;\nfn bad(value: str) -> unit {\n    if true { let moved: str = move(value); }\n    print(value);\n}\n", 5, "move後"),
        ("from std.ownership import move\ndef bad(value: str):\n    for number in range(2):\n        moved = move(value)\n", "from std.ownership import move;\nfn bad(value: str) -> unit {\n    for number in range(2) {\n        let moved: str = move(value);\n    }\n}\n", 4, "move後"),
    ] {
        // 独立Lowは宣言を一行に置くため、必要な入力ごとに元lineを指定する。
        fixture.write("bad.nagi", high);
        let high_error = fixture.checked("bad.nagi").expect_err(high);
        assert!(high_error.starts_with(&format!("line {line}:")) && high_error.contains(reason), "{high_error}");
        fixture.write("bad.low", low);
        let low_error = fixture.checked("bad.low").expect_err(low);
        let low_line = if high.contains("class Packet") { line - 1 } else if high.contains("if True") { line - 1 } else { line };
        assert!(low_error.starts_with(&format!("line {low_line}:")) && low_error.contains(reason), "{low_error}");
    }
    fixture.reject("from std.ownership import move\ndef bad(values: List[str]):\n    for value in values:\n        moved = move(value)\n", "from std.ownership import move;\nfn bad(values: List[str]) -> unit {\n    for value in values {\n        let moved: str = move(value);\n    }\n}\n", 4, "借用");
}

#[test]
fn moving_nested_view_containers_keeps_escape_and_temporary_rejections() {
    let fixture = Fixture::new();
    for (ty, expression) in [
        ("List[view[str]]", "[view(owner)]"),
        ("List[view[str]]?", "some([view(owner)])"),
        ("Result[List[view[str]], i64]", "ok([view(owner)])"),
    ] {
        fixture.reject(&format!("from std.ownership import move\ndef bad() -> {ty}:\n    owner = \"local\"\n    value: {ty} = {expression}\n    moved = move(value)\n    return moved\n"), &format!("from std.ownership import move;\nfn bad() -> {ty} {{\n    let owner: str = \"local\";\n    let value: {ty} = {expression};\n    let moved: {ty} = move(value);\n    return moved;\n}}\n"), 6, "view escapes");
    }
    fixture.reject("from std.ownership import move\ndef bad():\n    parts = move([view(\"temporary\")])\n", "from std.ownership import move;\nfn bad() -> unit {\n    let parts: List[view[str]] = move([view(\"temporary\")]);\n}\n", 3, "一時的な所有値");
}

#[test]
fn identity_moves_preserve_rhs_failure_drop_order_and_async_cancellation() {
    Fixture::new().run_three(
        "lifecycle",
        LIFE_HIGH,
        LIFE_LOW,
        LIFE_ADAPTER,
        LIFE_ASSERTIONS,
    );
}

const NORMAL_HIGH: &str = r#"from std.ownership import move as transfer
import std.ownership as ownership
class Pair:
    count: i64
class Packet:
    text: str
enum CopyTag:
    Number(value: i64)
    Empty
def move(value: i64) -> i64:
    return value + 10
def user_shadow() -> i64:
    return move(2)
def local_shadow() -> i64:
    transfer = 7
    return transfer
def copy_shapes(pair: Pair, tag: CopyTag, optional: i64?, part: view[str]) -> Pair:
    original = pair
    moved = transfer(pair)
    first = tag
    second = tag
    option = optional
    option2 = transfer(optional)
    text = part
    text2 = transfer(part)
    return original
def hints() -> i64:
    small: i8 = transfer(1)
    absent: i64? = transfer(None)
    return 1
def text(value: str) -> str:
    moved = ownership.move(value)
    moved = transfer(moved)
    return moved
def nested(value: str) -> str:
    return transfer(transfer(value))
def owned(value: owned[str]) -> owned[str]:
    moved = transfer(value)
    return moved
def values(value: List[str]) -> List[str]:
    moved = transfer(value)
    return moved
def shared(value: shared[str]) -> shared[str]:
    duplicate = clone_shared(value)
    moved = ownership.move(value)
    return moved
def result(value: Result[str, i64]) -> Result[str, i64]:
    moved = transfer(value)
    return moved
def field(value: Packet) -> str:
    return value.text
def fresh(value: str) -> Packet:
    return Packet(text=value)
def argument(value: str) -> str:
    return text(value)
def views(value: List[view[str]]) -> List[view[str]]:
    moved = transfer(value)
    return moved
def optional_views(value: List[view[str]]?) -> List[view[str]]?:
    moved = transfer(value)
    return moved
def result_views(value: Result[List[view[str]], i64]) -> Result[List[view[str]], i64]:
    moved = transfer(value)
    return moved
def replace_views(part: view[str]) -> List[view[str]]:
    local = copy(part)
    parts = [view(local)]
    moved = transfer(parts)
    moved = [part]
    return moved
async def answer(value: i64) -> i64:
    return value + 1
async def alias() -> i64:
    callback = answer
    selected = transfer(callback)
    return await selected(41)
"#;

// 手書きLowはHigh生成文字列を変換して作らない。
const NORMAL_LOW: &str = r#"from std.ownership import move as transfer;
import std.ownership as ownership;
record Pair { count: i64; }
record Packet { text: str; }
enum CopyTag { Number(value: i64); Empty; }
fn move(value: i64) -> i64 { return value + 10; }
fn user_shadow() -> i64 { return move(2); }
fn local_shadow() -> i64 { let transfer: i64 = 7; return transfer; }
fn copy_shapes(pair: Pair, tag: CopyTag, optional: i64?, part: view[str]) -> Pair {
    let original: Pair = pair;
    let moved: Pair = transfer(pair);
    let first: CopyTag = tag;
    let second: CopyTag = tag;
    let option: i64? = optional;
    let option2: i64? = transfer(optional);
    let text: view[str] = part;
    let text2: view[str] = transfer(part);
    return original;
}
fn hints() -> i64 { let small: i8 = transfer(1); let absent: i64? = transfer(None); return 1; }
fn text(value: str) -> str { let moved: str = ownership.move(value); moved = transfer(moved); return moved; }
fn nested(value: str) -> str { return transfer(transfer(value)); }
fn owned(value: owned[str]) -> owned[str] { let moved: owned[str] = transfer(value); return moved; }
fn values(value: List[str]) -> List[str] { let moved: List[str] = transfer(value); return moved; }
fn shared(value: shared[str]) -> shared[str] { let duplicate = clone_shared(value); let moved: shared[str] = ownership.move(value); return moved; }
fn result(value: Result[str, i64]) -> Result[str, i64] { let moved: Result[str, i64] = transfer(value); return moved; }
fn field(value: Packet) -> str { return value.text; }
fn fresh(value: str) -> Packet { return Packet(text=value); }
fn argument(value: str) -> str { return text(value); }
fn views(value: List[view[str]]) -> List[view[str]] { let moved: List[view[str]] = transfer(value); return moved; }
fn optional_views(value: List[view[str]]?) -> List[view[str]]? { let moved: List[view[str]]? = transfer(value); return moved; }
fn result_views(value: Result[List[view[str]], i64]) -> Result[List[view[str]], i64] { let moved: Result[List[view[str]], i64] = transfer(value); return moved; }
fn replace_views(part: view[str]) -> List[view[str]] {
    let local: str = copy(part);
    let parts: List[view[str]] = [view(local)];
    let moved: List[view[str]] = transfer(parts);
    moved = [part];
    return moved;
}
async fn answer(value: i64) -> i64 { return value + 1; }
async fn alias() -> i64 { let callback = answer; let selected = transfer(callback); return await selected(41); }
"#;

const NORMAL_ASSERTIONS: &str = r#"
#[test]
fn moved_payloads_and_copy_provenance() {
    assert_eq!(user_shadow(), 12);
    assert_eq!(local_shadow(), 7);
    assert_eq!(copy_shapes(Pair { count: 3 }, CopyTag::Number { value: 4 }, Some(5), "caller").count, 3);
    assert_eq!(hints(), 1);
    let input = String::from("Nagi");
    let pointer = input.as_ptr();
    let output = text(input);
    assert_eq!(output, "Nagi");
    assert_eq!(output.as_ptr(), pointer);
    assert_eq!(nested(String::from("nested")), "nested");
    assert_eq!(owned(String::from("owned")), "owned");
    let input = vec![String::from("list")];
    let pointer = input.as_ptr();
    let output = values(input);
    assert_eq!(output.as_ptr(), pointer);
    assert_eq!(output, vec!["list"]);
    let input = std::sync::Arc::new(String::from("shared"));
    let output = shared(std::sync::Arc::clone(&input));
    assert!(std::sync::Arc::ptr_eq(&input, &output));
    assert_eq!(std::sync::Arc::strong_count(&input), 2);
    assert_eq!(result(Ok(String::from("ok"))), Ok(String::from("ok")));
    assert_eq!(result(Err(9)), Err(9));
    assert_eq!(argument(field(fresh(String::from("field")))), "field");
    let owner = String::from("caller-owned");
    assert_eq!(views(vec![owner.as_str()]), vec!["caller-owned"]);
    assert_eq!(optional_views(Some(vec![owner.as_str()])), Some(vec!["caller-owned"]));
    assert_eq!(result_views(Ok(vec![owner.as_str()])), Ok(vec!["caller-owned"]));
    assert_eq!(replace_views(owner.as_str()), vec!["caller-owned"]);
    let mut task = std::pin::pin!(alias());
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    assert_eq!(std::future::Future::poll(task.as_mut(), &mut cx), std::task::Poll::Ready(42));
}
"#;

const LIFE_HIGH: &str = r#"from std.ownership import move
class Marker:
    id: i64
    text: str
@rust("native::marker")
extern def marker(id: i64) -> Marker
@rust("native::capture")
extern def capture(id: i64)
@rust("native::step")
extern def step() -> Result[Marker, i64]
@rust("native::crash")
extern def crash() -> Marker
@rust("native::pause")
extern async def pause()
def replace():
    destination = marker(1)
    source = marker(2)
    destination = move(source)
    capture(3)
def rhs_error() -> Result[unit, i64]:
    destination = marker(4)
    replacement = step()
    destination = try move(replacement)
    return ok(capture(9))
def rhs_panic():
    destination = marker(5)
    destination = move(crash())
def old_drop_panic():
    destination = marker(6)
    source = marker(7)
    destination = move(source)
    capture(8)
async def suspended(part: view[str]) -> List[view[str]]:
    before = marker(10)
    local = copy(part)
    parts = [view(local)]
    after = marker(11)
    parts = [part]
    transferred = move(parts)
    await pause()
    return transferred
"#;
const LIFE_LOW: &str = r#"from std.ownership import move;
record Marker { id: i64; text: str; }
@rust("native::marker") extern fn marker(id: i64) -> Marker;
@rust("native::capture") extern fn capture(id: i64) -> unit;
@rust("native::step") extern fn step() -> Result[Marker, i64];
@rust("native::crash") extern fn crash() -> Marker;
@rust("native::pause") extern async fn pause() -> unit;
fn replace() -> unit {
    let destination: Marker = marker(1);
    let source: Marker = marker(2);
    destination = move(source);
    capture(3);
}
fn rhs_error() -> Result[unit, i64] {
    let destination: Marker = marker(4);
    let replacement: Result[Marker, i64] = step();
    destination = try move(replacement);
    return ok(capture(9));
}
fn rhs_panic() -> unit { let destination: Marker = marker(5); destination = move(crash()); }
fn old_drop_panic() -> unit {
    let destination: Marker = marker(6);
    let source: Marker = marker(7);
    destination = move(source);
    capture(8);
}
async fn suspended(part: view[str]) -> List[view[str]] {
    let before: Marker = marker(10);
    let local: str = copy(part);
    let parts: List[view[str]] = [view(local)];
    let after: Marker = marker(11);
    parts = [part];
    let transferred: List[view[str]] = move(parts);
    await pause();
    return transferred;
}
"#;
const LIFE_ADAPTER: &str = r#"
static EVENTS: std::sync::Mutex<Vec<i64>> = std::sync::Mutex::new(Vec::new());
static PANIC_DROP: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
fn record(value: i64) { EVENTS.lock().unwrap().push(value); }
fn history() -> Vec<i64> { std::mem::take(&mut *EVENTS.lock().unwrap()) }
impl Clone for Marker {
    fn clone(&self) -> Self { record(900 + self.id); Self { id: self.id, text: self.text.clone() } }
}
impl Drop for Marker {
    fn drop(&mut self) {
        record(200 + self.id);
        if PANIC_DROP.compare_exchange(self.id, 0, std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst).is_ok() { panic!("controlled old Drop panic"); }
    }
}
mod native {
    pub fn marker(id: i64) -> super::Marker { super::record(100 + id); super::Marker { id, text: String::from("owned") } }
    pub fn capture(id: i64) { super::record(300 + id); }
    pub fn step() -> Result<super::Marker, i64> { super::record(400); Err(9) }
    pub fn crash() -> super::Marker { super::record(500); panic!("controlled RHS panic"); }
    pub async fn pause() { super::record(600); std::future::pending::<()>().await; }
}
"#;
const LIFE_ASSERTIONS: &str = r#"
#[test]
fn transfer_failure_and_future_cleanup() {
    replace();
    assert_eq!(history(), vec![101, 102, 201, 303, 202]);
    assert_eq!(rhs_error(), Err(9));
    assert_eq!(history(), vec![104, 400, 204]);
    assert!(std::panic::catch_unwind(rhs_panic).is_err());
    assert_eq!(history(), vec![105, 500, 205]);
    PANIC_DROP.store(6, std::sync::atomic::Ordering::SeqCst);
    assert!(std::panic::catch_unwind(old_drop_panic).is_err());
    assert_eq!(history(), vec![106, 107, 206, 207]);
    let owner = String::from("caller-owned");
    drop(suspended(owner.as_str()));
    assert!(history().is_empty(), "unpolled async body performed work");
    let mut pending = Box::pin(suspended(owner.as_str()));
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(std::future::Future::poll(pending.as_mut(), &mut cx).is_pending());
    assert_eq!(history(), vec![110, 111, 600]);
    drop(pending);
    assert_eq!(history(), vec![211, 210]);
    assert_eq!(owner, "caller-owned");
}
"#;
