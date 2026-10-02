use nagic::{check, emit, parser};
use std::{fs, path::PathBuf, process::Command};

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn length_and_size_casts_preserve_comparison_negation_and_numeric_contexts() {
    let source = r#"def owned_text_under(text: str, limit: i64) -> bool:
    return len(text) < limit

def borrowed_text_under(text: view[str], limit: i64) -> bool:
    return len(text) < limit

def list_under(values: view[i64], limit: i64) -> bool:
    return len(values) < limit

def size_under(limit: i64) -> bool:
    return size_of[i64]() < limit

def negative_text(text: view[str]) -> i64:
    return -len(text)

def negative_list(values: view[i64]) -> i64:
    return -len(values)

def negative_size() -> i64:
    return -size_of[i64]()

def numeric(values: view[i64]) -> i64:
    return len(values) * 10 + size_of[i64]()

def subtract_one(value: i64) -> i64:
    return value - 1

def call_with_length(text: view[str]) -> i64:
    return subtract_one(len(text))
"#;
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let mut rust = emit::rust(&low).unwrap();
    rust.push_str(
        r#"
#[test]
fn generated_cast_contexts() {
    let text = "あNagi";
    assert!(owned_text_under(String::from(text), 8));
    assert!(!owned_text_under(String::from(text), 7));
    assert!(borrowed_text_under(text, 8));
    assert!(!borrowed_text_under(text, 7));
    assert!(list_under(&[1, 2, 3], 4));
    assert!(!list_under(&[1, 2, 3], 3));
    assert!(size_under(9));
    assert!(!size_under(8));
    assert_eq!(negative_text(text), -7);
    assert_eq!(negative_text(""), 0);
    assert_eq!(negative_list(&[1, 2, 3]), -3);
    assert_eq!(negative_list(&[]), 0);
    assert_eq!(negative_size(), -8);
    assert_eq!(numeric(&[1, 2, 3]), 38);
    assert_eq!(call_with_length(text), 6);
}
"#,
    );

    // Only std is used by these builtins, so this exercises the generated
    // native program without a runtime crate, Cargo build, or shared target.
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "nagi-builtin-casts-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )));
    fs::create_dir(&fixture.0).unwrap();
    let generated = fixture.0.join("generated.rs");
    let binary = fixture
        .0
        .join(format!("generated{}", std::env::consts::EXE_SUFFIX));
    fs::write(&generated, rust).unwrap();
    let compiled = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2021", "--test", "-D", "unused-imports"])
        .arg(&generated)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let executed = Command::new(binary).output().unwrap();
    assert!(
        executed.status.success(),
        "{}{}",
        String::from_utf8_lossy(&executed.stdout),
        String::from_utf8_lossy(&executed.stderr)
    );
}
