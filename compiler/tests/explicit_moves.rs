#[path = "support/native_triple.rs"]
mod native_triple;
use nagic::emit;
use native_triple::Fixture;

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
        assert!(error.contains("非同期処理の戻り値"), "{error}");
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
        let low_line = if high.contains("class Packet") || high.contains("if True") { line - 1 } else { line };
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

#[test]
fn move_keeps_typed_constant_failures_and_profile_dependent_overflow() {
    let fixture = Fixture::new();
    for (import, operation) in [
        ("from std.ownership import move", "move"),
        ("from std.ownership import move as transfer", "transfer"),
    ] {
        for ty in ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"] {
            for operator in ["/", "%"] {
                for zero in ["0", "(1 - 1)"] {
                    fixture.reject(
                        &format!("{import}\ndef calculate(input: {ty}) -> {ty}:\n    return input {operator} {operation}({zero})\n"),
                        &format!("{import};\nfn calculate(input: {ty}) -> {ty} {{\n    return input {operator} {operation}({zero});\n}}\n"),
                        3, "E_CONST_ZERO_DIVISOR",
                    );
                }
                fixture.reject(
                    &format!("{import}\ndef calculate(input: {ty}) -> {ty}:\n    zero: {ty} = {operation}(2 - 2)\n    return input {operator} zero\n"),
                    &format!("{import};\nfn calculate(input: {ty}) -> {ty} {{\n    let zero: {ty} = {operation}(2 - 2);\n    return input {operator} zero;\n}}\n"),
                    4, "E_CONST_ZERO_DIVISOR",
                );
            }
        }
        for (ty, minimum) in [
            ("i8", "-128"),
            ("i16", "-32768"),
            ("i32", "-2147483648"),
            ("i64", "-9223372036854775808"),
        ] {
            for operator in ["/", "%"] {
                fixture.reject(
                    &format!("{import}\ndef calculate() -> {ty}:\n    return {operation}({minimum}) {operator} {operation}(-1)\n"),
                    &format!("{import};\nfn calculate() -> {ty} {{\n    return {operation}({minimum}) {operator} {operation}(-1);\n}}\n"),
                    3, "E_CONST_SIGNED_DIV_OVERFLOW",
                );
            }
        }
        // debug/release依存の中間値はKnown(0)へ変えず、現行受理を維持する。
        for (ty, maximum) in [
            ("i8", "127"),
            ("i16", "32767"),
            ("i32", "2147483647"),
            ("i64", "9223372036854775807"),
            ("u8", "255"),
            ("u16", "65535"),
            ("u32", "4294967295"),
            ("u64", "18446744073709551615"),
        ] {
            fixture.write("profile.nagi", &format!("{import}\ndef calculate() -> {ty}:\n    divisor: {ty} = {operation}({maximum} + 1)\n    return 1 / {operation}(divisor)\n"));
            let high = fixture.checked("profile.nagi").unwrap();
            fixture.write("profile.low", &emit::low(&high));
            fixture.checked("profile.low").unwrap();
            fixture.write("handwritten.low", &format!("{import};\nfn calculate() -> {ty} {{\n    let divisor: {ty} = {operation}({maximum} + 1);\n    return 1 / {operation}(divisor);\n}}\n"));
            fixture.checked("handwritten.low").unwrap();
        }
    }
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
def copy_input_remains_usable(value: i64) -> i64:
    moved = transfer(value)
    return value + moved
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
    return text(transfer(value))
def plain_argument_and_return(value: str) -> str:
    return text(value)
def plain_match_payload(value: Result[str, i64]) -> str:
    match value:
        case Ok(payload):
            return payload
        case Err(_):
            return "rejected"
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
fn copy_input_remains_usable(value: i64) -> i64 { let moved: i64 = transfer(value); return value + moved; }
fn hints() -> i64 { let small: i8 = transfer(1); let absent: i64? = transfer(None); return 1; }
fn text(value: str) -> str { let moved: str = ownership.move(value); moved = transfer(moved); return moved; }
fn nested(value: str) -> str { return transfer(transfer(value)); }
fn owned(value: owned[str]) -> owned[str] { let moved: owned[str] = transfer(value); return moved; }
fn values(value: List[str]) -> List[str] { let moved: List[str] = transfer(value); return moved; }
fn shared(value: shared[str]) -> shared[str] { let duplicate = clone_shared(value); let moved: shared[str] = ownership.move(value); return moved; }
fn result(value: Result[str, i64]) -> Result[str, i64] { let moved: Result[str, i64] = transfer(value); return moved; }
fn field(value: Packet) -> str { return value.text; }
fn fresh(value: str) -> Packet { return Packet(text=value); }
fn argument(value: str) -> str { return text(transfer(value)); }
fn plain_argument_and_return(value: str) -> str { return text(value); }
fn plain_match_payload(value: Result[str, i64]) -> str {
    match value {
        case Ok(payload) { return payload; }
        case Err(_) { return "rejected"; }
    }
}
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
    assert_eq!(copy_input_remains_usable(21), 42);
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
    let input = String::from("plain argument and return");
    let pointer = input.as_ptr();
    let output = plain_argument_and_return(input);
    assert_eq!(output, "plain argument and return");
    assert_eq!(output.as_ptr(), pointer);
    let input = String::from("plain match payload");
    let pointer = input.as_ptr();
    let output = plain_match_payload(Ok(input));
    assert_eq!(output, "plain match payload");
    assert_eq!(output.as_ptr(), pointer);
    assert_eq!(plain_match_payload(Err(9)), "rejected");
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
@rust("native::read")
extern def read(part: view[str])
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
def borrowed_observation():
    values = [marker(12)]
    count = len(move(values))
    capture(13)
    read(move(view("Nagi")))
    read(move(move(view("Nagi"))))
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
@rust("native::marker")
extern fn marker(id: i64) -> Marker;
@rust("native::capture")
extern fn capture(id: i64) -> unit;
@rust("native::read")
extern fn read(part: view[str]) -> unit;
@rust("native::step")
extern fn step() -> Result[Marker, i64];
@rust("native::crash")
extern fn crash() -> Marker;
@rust("native::pause")
extern async fn pause() -> unit;
fn replace() -> unit {
    let destination: Marker = marker(1);
    let source: Marker = marker(2);
    destination = move(source);
    capture(3);
}
fn borrowed_observation() -> unit {
    let values: List[Marker] = [marker(12)];
    let count: i64 = len(move(values));
    capture(13);
    read(move(view("Nagi")));
    read(move(move(view("Nagi"))));
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
    pub fn read(part: &str) { assert_eq!(part, "Nagi"); super::record(314); }
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
    borrowed_observation();
    assert_eq!(history(), vec![112, 212, 313, 314, 314], "moved Vec must retire after borrowed len, before capture");
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

#[test]
fn identity_move_materializes_copy_values_and_keeps_bare_place_comparison_loans() {
    let fixture = Fixture::new();
    let imports = "from std.ownership import move\nfrom std.ownership import move as transfer\nimport std.ownership as ownership\n";
    // 裸placeのPartialEq借用は維持。moveはby-valueなのでCopy入力から新temporaryを作る。
    // 旧9群は括弧だけの誤生成を前提にmove入力も借用placeと期待していた。
    for ty in ["i64?", "unit"] {
        let high_field = format!("{imports}class Pair:\n    number: {ty}\n    text: str\ndef take(pair: Pair) -> {ty}:\n    return pair.number\ndef compare(pair: Pair) -> bool:\n    return pair.number == take(pair)\n");
        let low_field = format!("{imports}record Pair {{ number: {ty}; text: str; }}\nfn take(pair: Pair) -> {ty} {{ return pair.number; }}\nfn compare(pair: Pair) -> bool {{\n    return pair.number == take(pair);\n}}\n");
        let high_index = format!("{imports}def take(values: List[{ty}]) -> {ty}:\n    return values[0]\ndef compare(values: List[{ty}]) -> bool:\n    return values[0] == take(values)\n");
        let low_index = format!("{imports}fn take(values: List[{ty}]) -> {ty} {{ return values[0]; }}\nfn compare(values: List[{ty}]) -> bool {{\n    return values[0] == take(values);\n}}\n");
        for (name, input, line) in [
            ("field.nagi", &high_field, 10),
            ("field.low", &low_field, 7),
            ("index.nagi", &high_index, 7),
            ("index.low", &low_index, 6),
        ] {
            fixture.write(name, input);
            let error = fixture.checked(name).expect_err(input);
            assert!(
                error.starts_with(&format!("line {line}:")),
                "{error}\n{input}"
            );
            assert!(error.contains("同じ式で先に参照"), "{error}\n{input}");
        }
    }
    let mut high = imports.to_owned();
    let mut low = imports.to_owned();
    let mut assertions =
        "#[test] fn copied_comparison_values_outlive_original_places() {\n".to_owned();
    for (type_index, (ty, value)) in [("i64?", "Some(7)"), ("unit", "()"), ("i64", "7")]
        .iter()
        .enumerate()
    {
        let pair = format!("Pair{type_index}");
        high.push_str(&format!("class {pair}:\n    number: {ty}\n    text: str\ndef take_field{type_index}(pair: {pair}) -> {ty}:\n    return pair.number\ndef take_index{type_index}(values: List[{ty}]) -> {ty}:\n    return values[0]\n"));
        low.push_str(&format!("record {pair} {{ number: {ty}; text: str; }}\nfn take_field{type_index}(pair: {pair}) -> {ty} {{ return pair.number; }}\nfn take_index{type_index}(values: List[{ty}]) -> {ty} {{ return values[0]; }}\n"));
        for (wrapper_index, wrapper) in [
            "move({place})",
            "transfer({place})",
            "ownership.move(move({place}))",
        ]
        .iter()
        .enumerate()
        {
            let field = wrapper.replace("{place}", "pair.number");
            let index = wrapper.replace("{place}", "values[0]");
            let suffix = format!("{type_index}_{wrapper_index}");
            high.push_str(&format!("def compare_field{suffix}(pair: {pair}) -> bool:\n    return {field} == take_field{type_index}(pair)\ndef compare_index{suffix}(values: List[{ty}]) -> bool:\n    return {index} == take_index{type_index}(values)\n"));
            low.push_str(&format!("fn compare_field{suffix}(pair: {pair}) -> bool {{ return {field} == take_field{type_index}(pair); }}\nfn compare_index{suffix}(values: List[{ty}]) -> bool {{ return {index} == take_index{type_index}(values); }}\n"));
            assertions.push_str(&format!("assert!(compare_field{suffix}({pair} {{ number: {value}, text: String::from(\"owner\") }}));\nassert!(compare_index{suffix}(vec![{value}]));\n"));
        }
    }
    assertions.push_str("}\n");
    fixture.run_three("comparison-values", &high, &low, "", &assertions);
}
