use nagic::{check, emit, parser, source};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

const REASON: &str = "sharedまたは借用resourceの非Copy fieldはmoveできません";

fn rejects(text: &str, high: bool) {
    let mut program = parser::parse(text, high).unwrap_or_else(|error| panic!("{text}\n{error}"));
    let error = check::check(&mut program).expect_err(text);
    assert!(error.contains(REASON), "{text}\n{error}");
}

#[test]
fn shared_field_moves_are_rejected_in_each_consuming_context_in_high_and_handwritten_low() {
    for ty in [
        "shared[Item]",
        "owned[shared[Item]]",
        "owned[owned[shared[Item]]]",
    ] {
        for (high_body, low_body, result) in [
            (
                "    text = value.text\n",
                "    let text: str = value.text;\n",
                "unit",
            ),
            ("    return value.text\n", "    return value.text;\n", "str"),
            (
                "    consume(value.text)\n",
                "    consume(value.text);\n",
                "unit",
            ),
            ("    value.text\n", "    value.text;\n", "unit"),
            (
                "    texts = [value.text]\n",
                "    let texts: List[str] = [value.text];\n",
                "unit",
            ),
            (
                "    item = Item(text=value.text, count=0)\n",
                "    let item: Item = Item(text=value.text, count=0);\n",
                "unit",
            ),
        ] {
            rejects(&format!("class Item:\n    text: str\n    count: i64\ndef consume(text: str):\n    print(text)\ndef bad(value: {ty}) -> {result}:\n{high_body}"), true);
            rejects(&format!("record Item {{ text: str; count: i64; }}\nfn consume(text: str) -> unit {{ print(text); }}\nfn bad(value: {ty}) -> {result} {{\n{low_body}}}\n"), false);
        }
    }
}

#[test]
fn nested_shared_parents_and_call_temporaries_cannot_supply_owned_fields() {
    rejects("class Item:\n    text: str\ndef bad() -> str:\n    return share(Item(text=\"Nagi\")).text\n", true);
    rejects(
        "record Item { text: str; }\nfn bad() -> str { return share(Item(text=\"Nagi\")).text; }\n",
        false,
    );
    for ty in [
        "shared[Item]",
        "owned[shared[Item]]",
        "owned[owned[shared[Item]]]",
    ] {
        rejects(&format!("class Item:\n    text: str\nclass Outer:\n    child: {ty}\ndef bad(value: Outer) -> str:\n    return value.child.text\n"), true);
        rejects(&format!("record Item {{ text: str; }}\nrecord Outer {{ child: {ty}; }}\nfn bad(value: Outer) -> str {{ return value.child.text; }}\n"), false);
        for (high_body, low_body) in [
            ("    return make().text\n", "    return make().text;\n"),
            (
                "    text = make().text\n    return text\n",
                "    let text: str = make().text;\n    return text;\n",
            ),
        ] {
            rejects(&format!("class Item:\n    text: str\n@rust(\"native::make\")\nextern def make() -> {ty}\ndef bad() -> str:\n{high_body}"), true);
            rejects(&format!("record Item {{ text: str; }}\n@rust(\"native::make\")\nextern fn make() -> {ty};\nfn bad() -> str {{\n{low_body}}}\n"), false);
        }
    }
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi-shared-field-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn valid_arc_moves_copy_fields_and_explicit_copies_run_in_high_and_independent_low() {
    let fixture = Fixture::new();
    fixture.write("main.nagi", VALID);
    fixture.write("native.rs", NATIVE);
    let mut high = source::load(&fixture.0.join("main.nagi"), true).unwrap();
    check::check(&mut high.program).unwrap();
    let low = emit::low(&high.program);
    let mut independent = parser::parse(&low, false).unwrap();
    check::check(&mut independent).unwrap();
    fixture.write("main.low", &low);
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
            .args([
                "run",
                entry,
                "--rust",
                "native.rs",
                "--out",
                "build",
                "--no-project",
            ])
            .env("NAGI_NATIVE_TARGET_DIR", &target)
            .env("CARGO_NET_OFFLINE", "true")
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
            "14\nNagi\n7\nNagi\n14\nNagi\n14\nNagi\n14\nNagi\nNagi\nNagi\nNagi\nNagi\nNagi\nNagi"
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("native: "));
    }
}

#[test]
fn invalid_shared_field_moves_fail_before_check_build_or_run_needs_cargo() {
    let fixture = Fixture::new();
    for (entry, text, line) in [
        ("bad.nagi", "class Item:\n    text: str\ndef bad(value: owned[shared[Item]]) -> str:\n    return value.text\n", 4),
        ("bad.low", "record Item { text: str; }\n@rust(\"native::make\")\nextern fn make() -> owned[shared[Item]];\nfn bad() -> str { return make().text; }\n", 4),
    ] {
        fixture.write(entry, text);
        for command in ["check", "build", "run"] {
            let output = Command::new(env!("CARGO_BIN_EXE_nagic")).current_dir(&fixture.0)
                .args([command, entry, "--out", "build", "--no-project"])
                .env("PATH", "").env("NAGI_ROOT", fixture.0.join("missing-runtime")).output().unwrap();
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "{command} accepted {entry}");
            assert!(error.contains(REASON) && error.contains(&format!("{entry}:{line}")), "{error}");
            assert!(!error.contains("Rust backend") && !error.contains("Cargo"), "{error}");
            assert!(!fixture.0.join("build/src/main.rs").exists());
        }
    }
}

const VALID: &str = r#"class Item:
    text: str
    count: i64
class Holder:
    child: shared[Item]
class OwnedHolder:
    child: owned[shared[Item]]
@rust("native::make_shared")
extern def make_shared() -> shared[Item]
@rust("native::make_shared")
extern def make_owned_shared() -> owned[shared[Item]]
@rust("native::make_shared")
extern def make_double_owned_shared() -> owned[owned[shared[Item]]]
@rust("native::make_item")
extern def make_owned_item() -> owned[Item]
def move_arc(holder: Holder) -> shared[Item]:
    return holder.child
def move_owned_arc(holder: OwnedHolder) -> owned[shared[Item]]:
    return holder.child
def move_item(item: owned[Item]) -> str:
    return item.text
def move_plain(item: Item) -> str:
    return item.text
def main():
    local = share(Item(text="Nagi", count=7))
    print(local.count + local.count)
    print(copy(view(local.text)))
    print(share(Item(text="Nagi", count=7)).count)
    print(copy(view(share(Item(text="Nagi", count=7)).text)))
    plain = make_shared()
    print(plain.count + plain.count)
    print(copy(view(plain.text)))
    wrapped = make_owned_shared()
    print(wrapped.count + wrapped.count)
    print(copy(view(wrapped.text)))
    double = make_double_owned_shared()
    print(double.count + double.count)
    print(copy(view(double.text)))
    moved = move_arc(Holder(child=make_shared()))
    print(copy(view(moved.text)))
    moved_owned = move_owned_arc(OwnedHolder(child=make_owned_shared()))
    print(copy(view(moved_owned.text)))
    print(move_item(make_owned_item()))
    print(move_plain(Item(text="Nagi", count=7)))
    native_temporary = make_owned_item().text
    print(native_temporary)
    plain_temporary = Item(text="Nagi", count=7).text
    print(plain_temporary)
"#;
const NATIVE: &str = r#"pub fn make_item() -> super::Item { super::Item { text: "Nagi".into(), count: 7 } }
pub fn make_shared() -> std::sync::Arc<super::Item> { std::sync::Arc::new(make_item()) }
"#;
