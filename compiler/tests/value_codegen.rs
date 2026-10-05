#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, parser};
use std::{fs, path::PathBuf, process::Command};

static FIXTURE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
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
        .args(["--edition=2021", "--test", "-D", "unused-imports"])
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
fn try_result_consumption_preserves_payloads_reinitialization_and_errors() {
    let source = "def extract(outcome: Result[str, i64]) -> Result[str, i64]:\n    return ok(try outcome)\ndef repeated() -> Result[i64, i64]:\n    outcome: Result[str, i64] = ok(\"first\")\n    total = 0\n    for number in range(2):\n        value = try outcome\n        total += len(value)\n        outcome = ok(\"next\")\n    return ok(total)\ndef borrowed(outcome: Result[view[str], i64]) -> Result[view[str], i64]:\n    return ok(try outcome)\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let mut code = emit::rust(&checked_emission::seal(&low)).unwrap();
    code.push_str(
        r#"
#[test] fn generated_try_results() {
    assert_eq!(extract(Ok(String::from("owned"))), Ok(String::from("owned")));
    assert_eq!(extract(Err(7)), Err(7));
    assert_eq!(repeated(), Ok(9));
    let text = String::from("borrowed");
    assert_eq!(borrowed(Ok(text.as_str())), Ok("borrowed"));
    assert_eq!(borrowed(Err(8)), Err(8));
}
"#,
    );
    compile_and_run(code);
}

#[test]
fn inferred_lists_compile_with_a_moving_first_call_or_wrapper() {
    let source = "def identity(value: str) -> str:\n    return value\ndef strings() -> List[str]:\n    text = \"Nagi\"\n    values = [identity(text)]\n    return values\ndef optional() -> List[str?]:\n    text = \"Nagi\"\n    values = [some(text)]\n    return values\ndef nested() -> List[List[str]]:\n    text = \"Nagi\"\n    values = [[identity(text)]]\n    return values\ndef extracted() -> Result[List[str], i64]:\n    outcome: Result[str, i64] = ok(\"Nagi\")\n    values = [try outcome]\n    return ok(values)\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let mut code = emit::rust(&checked_emission::seal(&low)).unwrap();
    code.push_str(
        r#"
#[test] fn generated_inferred_lists() {
    assert_eq!(strings(), vec![String::from("Nagi")]);
    assert_eq!(optional(), vec![Some(String::from("Nagi"))]);
    assert_eq!(nested(), vec![vec![String::from("Nagi")]]);
    assert_eq!(extracted(), Ok(vec![String::from("Nagi")]));
}
"#,
    );
    compile_and_run(code);
}
