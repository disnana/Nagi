use nagic::{check, emit, parser};
use std::{fs, path::PathBuf, process::Command};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi integer arithmetic {} {}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn compile(&self, file: &str, release: bool) -> std::process::Output {
        let mut command = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
        command.args(["--edition=2021", "--test"]);
        if release {
            command.args(["-C", "opt-level=3", "-C", "overflow-checks=off"]);
        } else {
            command.args(["-C", "overflow-checks=on"]);
        }
        command
            .arg(self.0.join(file))
            .arg("-o")
            .arg(self.binary(release))
            .output()
            .unwrap()
    }
    fn binary(&self, release: bool) -> PathBuf {
        self.0.join(format!(
            "{}{}",
            if release { "release" } else { "debug" },
            std::env::consts::EXE_SUFFIX
        ))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const SOURCE: &str = r#"def addition() -> u8:
    value: u8 = 255 + 1
    return value

def subtraction() -> u8:
    value: u8 = 0 - 1
    return value

def multiplication() -> u8:
    value: u8 = 200 * 2
    return value

async def asynchronous_addition() -> u8:
    value: u8 = 255 + 1
    return value
"#;

#[test]
fn constant_integer_arithmetic_preserves_release_wrapping_and_debug_panics() {
    let mut high = parser::parse(SOURCE, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let high_rust = emit::rust(&high).unwrap();
    assert_eq!(high_rust, emit::rust(&low).unwrap());
    let f = Fixture::new();
    for release in [true, false] {
        let assertions = if release {
            "assert_eq!(addition(), 0); assert_eq!(subtraction(), 255); assert_eq!(multiplication(), 144);"
        } else {
            "assert!(std::panic::catch_unwind(addition).is_err()); assert!(std::panic::catch_unwind(subtraction).is_err()); assert!(std::panic::catch_unwind(multiplication).is_err());"
        };
        fs::write(
            f.0.join("generated.rs"),
            format!("{high_rust}\n#[test] fn arithmetic() {{ {assertions} }}\n"),
        )
        .unwrap();
        let output = f.compile("generated.rs", release);
        assert!(output.status.success(), "{output:?}");
        let output = Command::new(f.binary(release)).output().unwrap();
        assert!(output.status.success(), "{output:?}");
    }
}

#[test]
fn native_adapter_constant_overflow_keeps_rust_diagnostics() {
    let mut high = parser::parse(SOURCE, true).unwrap();
    check::check(&mut high).unwrap();
    let rust = emit::rust(&high).unwrap();
    let f = Fixture::new();
    fs::write(
        f.0.join("adapter.rs"),
        "pub fn overflow() -> u8 { let value: u8 = 255 + 1; value }\n",
    )
    .unwrap();
    fs::write(
        f.0.join("generated.rs"),
        format!("{rust}\n#[path=\"adapter.rs\"] mod native;\n"),
    )
    .unwrap();
    let output = f.compile("generated.rs", true);
    assert!(!output.status.success());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(
        error.contains("arithmetic operation will overflow"),
        "{error}"
    );
    assert!(error.contains("adapter.rs"), "{error}");
}
