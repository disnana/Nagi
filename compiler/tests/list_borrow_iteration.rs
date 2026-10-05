#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{ast::*, check, emit, parser, source, symbols};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi-list-borrow-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn native(&self, name: &str, program: &Program, assertions: &str) {
        let path = self.0.join(format!("{name}.rs"));
        let binary = self
            .0
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        let rust = format!(
            "{}\n{assertions}",
            emit::rust(&checked_emission::seal(program)).unwrap()
        );
        fs::write(&path, rust).unwrap();
        let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2021", "--test", "-O"])
            .arg(path)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(binary).output().unwrap();
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

fn checked(source: &str, high: bool) -> Program {
    let mut program = parser::parse(source, high).unwrap();
    check::check(&mut program).unwrap();
    program
}

const HIGH: &str = r#"enum Kind:
    Stock
class Item:
    sku: str
    quantity: i64
    kind: Kind
class Bucket:
    title: str
    items: List[Item]
def total(items: List[Item]) -> i64:
    answer = 0
    for item in items:
        answer += len(item.sku) * item.quantity
        if view(item.sku) == "a":
            answer += 1
    append(items, Item(sku="end", quantity=0, kind=Kind.Stock))
    return answer + len(items)
def borrowed_total(items: view[Item]) -> i64:
    answer = 0
    for item in items:
        answer += len(item.sku) * item.quantity
    return answer
def strings() -> i64:
    values = ["a", "bb"]
    answer = 0
    output: List[str] = []
    for text in values:
        answer += len(text)
        if text == "a":
            answer += 1
        append(output, copy(view(text)))
    append(values, "ccc")
    return answer + len(values) + len(output)
def nested() -> i64:
    values = [["a", "bb"], ["ccc"]]
    answer = 0
    for texts in values:
        for text in texts:
            answer += len(text)
    return answer + len(values)
def temporary() -> i64:
    answer = 0
    for item in [Item(sku="a", quantity=2, kind=Kind.Stock)]:
        answer += len(item.sku) * item.quantity
    return answer
def primitives() -> i64:
    answer = 0
    for value in [1, 2]:
        value += 10
        answer += value
    return answer
def disjoint_field() -> i64:
    bucket = Bucket(title="owned", items=[Item(sku="ab", quantity=1, kind=Kind.Stock)])
    for item in bucket.items:
        title = bucket.title
        return len(title) + len(item.sku)
    return 0
"#;

const NATIVE_ASSERTIONS: &str = r#"
struct CountingAllocator;
thread_local! {
    static TRACK: ::std::cell::Cell<bool> = const { ::std::cell::Cell::new(false) };
    static ALLOCATIONS: ::std::cell::Cell<usize> = const { ::std::cell::Cell::new(0) };
}
fn count_allocation() {
    let _ = TRACK.try_with(|track| {
        if track.get() {
            let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
        }
    });
}
unsafe impl ::std::alloc::GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: ::std::alloc::Layout) -> *mut u8 {
        count_allocation();
        unsafe { ::std::alloc::GlobalAlloc::alloc(&::std::alloc::System, layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: ::std::alloc::Layout) {
        unsafe { ::std::alloc::GlobalAlloc::dealloc(&::std::alloc::System, pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: ::std::alloc::Layout, size: usize) -> *mut u8 {
        count_allocation();
        unsafe { ::std::alloc::GlobalAlloc::realloc(&::std::alloc::System, pointer, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
#[test]
fn reads_owned_records_without_a_clone_implementation() {
    let items = vec![
        Item { sku: "a".into(), quantity: 2, kind: Kind::Stock },
        Item { sku: "bb".into(), quantity: 3, kind: Kind::Stock },
    ];
    ALLOCATIONS.with(|count| count.set(0));
    TRACK.with(|track| track.set(true));
    let borrowed_answer = borrowed_total(&items);
    TRACK.with(|track| track.set(false));
    assert_eq!(borrowed_answer, 8);
    // Only this test thread is counted. Source strings and the List were
    // allocated above; the typed read-only loop performs no new allocations.
    assert_eq!(ALLOCATIONS.with(|count| count.get()), 0);
    assert_eq!(total(items), 12);
    assert_eq!(strings(), 9);
    assert_eq!(nested(), 8);
    assert_eq!(temporary(), 2);
    assert_eq!(primitives(), 23);
    assert_eq!(disjoint_field(), 7);
}
"#;

#[test]
fn noncopy_iteration_is_native_and_survives_saved_low() {
    let high = checked(HIGH, true);
    let low = checked(&emit::low(&high), false);
    let fixture = Fixture::new();
    for (name, program) in [("high", &high), ("saved-low", &low)] {
        let rust = emit::rust(&checked_emission::seal(program)).unwrap();
        assert!(!rust.contains(".iter().cloned()"));
        fixture.native(name, program, NATIVE_ASSERTIONS);
    }

    let manual_low = checked(
        r#"enum Kind { Stock; }
record Item { sku: str; quantity: i64; kind: Kind; }
fn answer() -> i64 {
    let items = [
        Item(sku="a", quantity=2, kind=Kind.Stock),
        Item(sku="bb", quantity=3, kind=Kind.Stock)
    ];
    let answer = 0;
    for item in view(items) {
        answer += len(view(item.sku)) * item.quantity;
    }
    append(items, Item(sku="c", quantity=0, kind=Kind.Stock));
    return answer + len(items);
}
"#,
        false,
    );
    fixture.native(
        "handwritten-low",
        &manual_low,
        "#[test] fn reads_borrowed_items() { assert_eq!(answer(), 11); }",
    );
}

fn rejects_both(source: &str, expected: &str) {
    let untyped = parser::parse(source, true).unwrap();
    let low = emit::low(&untyped);
    for (is_high, code) in [(true, source), (false, low.as_str())] {
        let mut program = parser::parse(code, is_high).unwrap();
        let error = check::check(&mut program).expect_err(code);
        assert!(
            error.contains(expected),
            "{code}\n{error}; expected {expected}"
        );
    }
}

#[test]
fn borrowed_elements_cannot_move_reassign_or_mutate() {
    let header = "class Item:\n    sku: str\n    quantity: i64\ndef consume(value: Item):\n    print(value.quantity)\ndef consume_text(value: str):\n    print(value)\ndef main():\n    items = [Item(sku=\"a\", quantity=1)]\n";
    for body in [
        "    for item in items:\n        consume(item)\n",
        "    for item in items:\n        owned = item\n",
        "    for item in items:\n        text = item.sku\n",
        "    for item in items:\n        consume_text(item.sku)\n",
        "    for item in items:\n        item = Item(sku=\"b\", quantity=2)\n",
    ] {
        rejects_both(&format!("{header}{body}"), "読み取り専用借用");
    }
    rejects_both(
        "def main():\n    values = [[1]]\n    for numbers in values:\n        append(numbers, 2)\n",
        "読み取り専用借用",
    );
    rejects_both("enum Message:\n    Text(value: str)\ndef main():\n    messages = [Message.Text(\"a\")]\n    for message in messages:\n        match message:\n            case Message.Text(text):\n                print(text)\n", "読み取り専用借用");
}

#[test]
fn an_active_element_pins_its_owner_and_aliases() {
    let header = "class Item:\n    sku: str\ndef consume(values: List[Item]):\n    print(len(values))\ndef main():\n    items = [Item(sku=\"a\")]\n";
    for iterable in ["items", "view(items)", "alias"] {
        let alias = if iterable == "alias" {
            "    alias = view(items)\n"
        } else {
            ""
        };
        for action in [
            "        append(items, Item(sku=\"b\"))\n",
            "        items = [Item(sku=\"b\")]\n",
            "        consume(items)\n",
        ] {
            rejects_both(
                &format!(
                    "{header}{alias}    for item in {iterable}:\n{action}        print(item.sku)\n"
                ),
                "参照",
            );
        }
    }
    // Ending a body removes a Copy iterator's backedge loan, but a reference
    // still pins its owner while the body reads it.
    rejects_both("class Item:\n    sku: str\ndef first(items: List[Item]) -> i64:\n    for item in items:\n        items = [Item(sku=\"b\")]\n        return len(item.sku)\n    return 0\n", "参照");
    rejects_both("class Item:\n    sku: str\nclass Bucket:\n    items: List[Item]\n    title: str\ndef main():\n    bucket = Bucket(items=[Item(sku=\"a\")], title=\"owned\")\n    for item in bucket.items:\n        moved = bucket.items\n        print(item.sku)\n", "参照");
}

#[test]
fn borrowed_elements_and_their_field_views_cannot_escape() {
    rejects_both("class Item:\n    sku: str\ndef first(items: List[Item]) -> Item:\n    for item in items:\n        return item\n    return Item(sku=\"empty\")\n", "読み取り専用借用");
    rejects_both("class Item:\n    sku: str\ndef first(items: view[Item]) -> view[str]:\n    for item in items:\n        return view(item.sku)\n    return view(\"empty\")\n", "escapes its lifetime");
    rejects_both("class Item:\n    sku: str\ndef main():\n    items = [Item(sku=\"a\")]\n    seed = \"empty\"\n    saved: view[str] = view(seed)\n    for item in items:\n        saved = view(item.sku)\n    print(saved)\n", "scopeの外へ保存");
    rejects_both("def main():\n    values = [[\"a\"]]\n    seed = \"empty\"\n    saved: view[str] = view(seed)\n    for texts in values:\n        for text in texts:\n            saved = view(text)\n    print(saved)\n", "scopeの外へ保存");
    rejects_both("class Item:\n    sku: str\ndef main():\n    items = [Item(sku=\"a\")]\n    seed = \"empty\"\n    output = [view(seed)]\n    for item in items:\n        append(output, view(item.sku))\n    print(len(output))\n", "scopeの外へ保存");
}

#[test]
fn symbols_distinguish_borrowed_record_locals_and_fields() {
    let fixture = Fixture::new();
    let file = fixture.0.join("main.nagi");
    fs::write(&file, "class Item:\n    sku: str\n    quantity: i64\ndef main():\n    items = [Item(sku=\"a\", quantity=1)]\n    for item in items:\n        print(item.sku)\n        print(item.quantity)\n").unwrap();
    let sources = source::load(&file, true).unwrap();
    let index = symbols::index(&sources, &[&sources.program]).unwrap();
    let locals = index["locals"].as_array().unwrap();
    let items = locals
        .iter()
        .filter(|value| value["name"] == "item")
        .collect::<Vec<_>>();
    assert!(!items.is_empty());
    assert!(items.iter().all(|value| value["type"] == "Item"
        && value["borrowed"] == true
        && value["readonly"] == true));
    let expression = index["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["type"] == "Item" && value["borrowed"] == true)
        .unwrap();
    assert!(expression["fields"]
        .as_array()
        .unwrap()
        .iter()
        .all(|field| field["readonly"] == true));
}
