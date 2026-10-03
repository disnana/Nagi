use nagic::{check, emit, parser};
use std::{fs, path::PathBuf, process::Command};

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const SOURCE: &str = r#"def inspect(value: Option[Option[i64]]) -> i64:
    match value:
        case None:
            return -1
        case Some(inner):
            match inner:
                case None:
                    return -2
                case Some(number):
                    return number

def answer() -> i64:
    absent: Option[Option[i64]] = None
    empty: Option[Option[i64]] = some(None)
    full: Option[Option[i64]] = some(some(42))
    return inspect(absent) + inspect(empty) + inspect(full)
"#;

#[test]
fn nested_options_keep_distinct_none_and_some_values_through_independent_low() {
    let mut high = parser::parse(SOURCE, true).unwrap();
    check::check(&mut high).unwrap();
    let low_source = emit::low(&high);
    let mut low =
        parser::parse(&low_source, false).unwrap_or_else(|error| panic!("{error}\n{low_source}"));
    check::check(&mut low).unwrap();
    let rust = emit::rust(&high).unwrap();
    assert_eq!(rust, emit::rust(&low).unwrap());

    let fixture = Fixture(std::env::temp_dir().join(format!(
        "nagi nested options {} {}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )));
    fs::create_dir_all(&fixture.0).unwrap();
    let source = fixture.0.join("generated.rs");
    let binary = fixture
        .0
        .join(format!("generated{}", std::env::consts::EXE_SUFFIX));
    fs::write(
        &source,
        format!(
            "{rust}\n#[test] fn nested_values() {{ assert_eq!(inspect(None), -1); assert_eq!(inspect(Some(None)), -2); assert_eq!(inspect(Some(Some(42))), 42); assert_eq!(answer(), 39); }}\n"
        ),
    )
    .unwrap();
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
