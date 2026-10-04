use nagic::{check, emit, parser, source};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi-owned-copy-{}-{}",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
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
fn owned_scalar_fields_copy_in_native_high_and_independent_low() {
    let fixture = Fixture::new();
    fixture.write("owned_copy.nagi", SCALARS);
    fixture.write("native.rs", NATIVE);
    let mut high = source::load(&fixture.0.join("owned_copy.nagi"), true).unwrap();
    check::check(&mut high.program).unwrap();
    let low_text = emit::low(&high.program);
    let mut independent = parser::parse(&low_text, false).unwrap();
    check::check(&mut independent).unwrap();
    assert_eq!(
        high.program.classes[0].fields,
        independent.classes[0].fields
    );
    assert_eq!(
        high.program.classes[1].fields,
        independent.classes[1].fields
    );
    assert_eq!(
        high.program.enums[0].variants[0].fields,
        independent.enums[0].variants[0].fields
    );
    fixture.write("owned_copy.low", &low_text);
    let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("native-target")
        });
    for name in ["owned_copy.nagi", "owned_copy.low"] {
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(&fixture.0)
            .args([
                "run",
                name,
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
            "{name}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.lines().any(|line| line.starts_with("native: ")));
        assert_eq!(stdout.replace("\r\n", "\n").trim(), "82\n84\n86");
    }
}

#[test]
fn owned_string_and_list_fields_remain_moves() {
    for inner in ["str", "List[i64]", "owned[str]", "Option[owned[List[i64]]]"] {
        for declaration in [
            format!("class Item:\n    value: owned[{inner}]\n"),
            format!("enum Item:\n    Value(value: owned[{inner}])\n    Empty\n"),
        ] {
            let once = format!(
                "{declaration}\ndef consume(item: Item):\n    return assert_true(True)\ndef repeat(item: Item):\n    consume(item)\n"
            );
            let mut high = parser::parse(&once, true).unwrap();
            check::check(&mut high).unwrap();
            let mut low = parser::parse(&emit::low(&high), false).unwrap();
            check::check(&mut low).unwrap();
            for program in [&high, &low] {
                let rust = emit::rust(program).unwrap();
                assert!(!rust.contains("Clone, Copy"), "{inner}: {rust}");
            }
            let twice = format!("{once}    consume(item)\n");
            let high = parser::parse(&twice, true).unwrap();
            for mut program in [
                high.clone(),
                parser::parse(&emit::low(&high), false).unwrap(),
            ] {
                let error = check::check(&mut program).unwrap_err();
                assert!(error.contains("move後"), "{inner}: {error}");
            }
        }
    }
}

const SCALARS: &str = r#"class Item:
    value: owned[i64]
class Nullable:
    value: Option[owned[i64]]
enum Wrapped:
    Value(value: owned[owned[i64]])
    Empty

@rust("native::make_item")
extern def make_item() -> Item
@rust("native::read_item")
extern def read_item(item: Item) -> i64
@rust("native::make_nullable")
extern def make_nullable() -> Nullable
@rust("native::read_nullable")
extern def read_nullable(item: Nullable) -> i64
@rust("native::make_wrapped")
extern def make_wrapped() -> Wrapped
@rust("native::read_wrapped")
extern def read_wrapped(item: Wrapped) -> i64

def repeat_item(item: Item) -> i64:
    first = read_item(item)
    return first + read_item(item)
def repeat_nullable(item: Nullable) -> i64:
    first = read_nullable(item)
    return first + read_nullable(item)
def repeat_wrapped(item: Wrapped) -> i64:
    first = read_wrapped(item)
    return first + read_wrapped(item)
def main():
    print(repeat_item(make_item()))
    print(repeat_nullable(make_nullable()))
    print(repeat_wrapped(make_wrapped()))
"#;

const NATIVE: &str = r#"pub fn make_item() -> super::Item { super::Item { value: 41 } }
pub fn read_item(item: super::Item) -> i64 { item.value }
pub fn make_nullable() -> super::Nullable { super::Nullable { value: Some(42) } }
pub fn read_nullable(item: super::Nullable) -> i64 { item.value.unwrap() }
pub fn make_wrapped() -> super::Wrapped { super::Wrapped::Value { value: 43 } }
pub fn read_wrapped(item: super::Wrapped) -> i64 {
    match item { super::Wrapped::Value { value } => value, super::Wrapped::Empty => 0 }
}
"#;
