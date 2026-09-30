use nagic::{check, emit, parser};
use std::{fs, path::PathBuf, process::Command};

static FIXTURE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn nested_views_compile_and_copy_strings_bytes_and_lists() {
    let mut high = parser::parse(
        "def text_copy(data: str) -> str:\n    return copy(view(data))\n\
         def bytes_copy(data: bytes) -> bytes:\n    return copy(view(data))\n\
         def list_copy(data: List[i64]) -> List[i64]:\n    return copy(view(data))\n\
         def text_len(data: str) -> i64:\n    return len(view(data))\n",
        true,
    )
    .unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let mut code = emit::rust(&low).unwrap();
    // These functions only use std. Supply empty modules for the emitter's
    // unconditional runtime imports, so this regression needs no Cargo build.
    code.push_str(
        "\nmod nagi_runtime { pub mod axum {} pub mod serde {} pub mod serde_json {} }\n\
         #[test] fn generated_values() {\n\
         assert_eq!(text_copy(String::from(\"Nagi\")), \"Nagi\");\n\
         assert_eq!(bytes_copy(vec![0, 128, 255]), vec![0, 128, 255]);\n\
         assert_eq!(list_copy(vec![1, 2, 3]), vec![1, 2, 3]);\n\
         assert_eq!(text_len(String::from(\"あ\")), 3);\n}\n",
    );
    compile_and_run(code);
}

fn compile_and_run(code: String) {
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "nagi-codegen-{}-{}-{}",
        std::process::id(),
            FIXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )));
    fs::create_dir(&fixture.0).unwrap();
    let source = fixture.0.join("generated.rs");
    let binary = fixture
        .0
        .join(format!("generated{}", std::env::consts::EXE_SUFFIX));
    fs::write(&source, code).unwrap();
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2021", "--test"])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
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

#[test]
fn result_match_compiles_and_runs_owned_and_borrowed_payloads() {
    let source = r#"def choose(r: Result[i64, i64]) -> i64:
    match r:
        case Ok(number):
            number += 1
            return number
        case Err(reason):
            return reason

def payload(r: Result[str, i64]) -> str:
    match r:
        case Ok(text):
            return text
        case Err(_):
            return "fallback"

def borrowed(r: Result[view[str], i64]) -> str:
    match r:
        case Ok(part):
            return copy(part)
        case Err(_):
            return "fallback"
"#;
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let mut code = emit::rust(&low).unwrap();
    code.push_str(
        r#"
mod nagi_runtime { pub mod axum {} pub mod serde {} pub mod serde_json {} }
#[test] fn generated_matches() {
    assert_eq!(choose(Ok(41)), 42);
    assert_eq!(choose(Err(-1)), -1);
    assert_eq!(payload(Ok(String::from("owned"))), "owned");
    assert_eq!(payload(Err(1)), "fallback");
    let text = String::from("borrowed");
    assert_eq!(borrowed(Ok(text.as_str())), "borrowed");
    assert_eq!(borrowed(Err(1)), "fallback");
}
"#,
    );
    compile_and_run(code);
}
