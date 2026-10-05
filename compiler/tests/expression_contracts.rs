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

fn compile_and_run(source: &str) -> String {
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();

    let fixture = Fixture(std::env::temp_dir().join(format!(
        "nagi-expression-contracts-{}-{}",
        std::process::id(),
        FIXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    )));
    fs::create_dir(&fixture.0).unwrap();
    let generated = fixture.0.join("generated.rs");
    let executable = fixture
        .0
        .join(format!("generated{}", std::env::consts::EXE_SUFFIX));
    fs::write(
        &generated,
        emit::rust(&checked_emission::seal(&low)).unwrap(),
    )
    .unwrap();
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2021", "-D", "unused-imports"])
        .arg(&generated)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(executable).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn inferred_i64_literals_keep_their_type_without_a_rust_type_context() {
    let output = compile_and_run(
        r#"def main():
    print(2147483648)
    print(9223372036854775807)
    print(-9223372036854775808)
    print(2147483648 + 1)
    assert_true(2147483648 == 2147483648)
    assert_true(-2147483648 / -1 == 2147483648)
    print(len([2147483648, 9223372036854775807]))
"#,
    );
    assert_eq!(
        output.lines().collect::<Vec<_>>(),
        [
            "2147483648",
            "9223372036854775807",
            "-9223372036854775808",
            "2147483649",
            "2",
        ]
    );
}

#[test]
fn literal_suffixes_preserve_contextual_numeric_types_and_negative_boundaries() {
    let output = compile_and_run(
        r#"def signed_byte() -> i8:
    return -128
def unsigned_byte() -> u8:
    return 255
def signed_word() -> i16:
    return -32768
def signed_integer() -> i32:
    return -2147483648
def unsigned_integer() -> u32:
    return 4294967295
def unsigned_long() -> u64:
    return 18446744073709551615
def single_precision() -> f32:
    return 1.5
def main():
    print(signed_byte())
    print(unsigned_byte())
    print(signed_word())
    print(signed_integer())
    print(unsigned_integer())
    print(unsigned_long())
    print(single_precision())
    assert_true(single_precision() == 1.5)
"#,
    );
    assert_eq!(
        output.lines().collect::<Vec<_>>(),
        [
            "-128",
            "255",
            "-32768",
            "-2147483648",
            "4294967295",
            "18446744073709551615",
            "1.5",
        ]
    );
}
