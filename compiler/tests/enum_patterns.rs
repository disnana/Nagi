#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, parser};
use std::{fs, path::PathBuf, process::Command};

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const SOURCE: &str = r#"enum Packet:
    Value(reference: i64, type: i64, __nagi_ident_0: i64)
    Empty

def same_names(packet: Packet) -> i64:
    match packet:
        case Packet.Value(reference, type, __nagi_ident_0):
            reference += 1
            type += 2
            __nagi_ident_0 += 3
            return reference + type + __nagi_ident_0
        case Packet.Empty:
            return 0

def renamed(packet: Packet) -> i64:
    match packet:
        case Packet.Value(first, other, type):
            first += 10
            return first + other + type
        case Packet.Empty:
            return 0

def ignored(packet: Packet) -> i64:
    match packet:
        case Packet.Value(reference, _, last):
            reference += 4
            return reference + last
        case Packet.Empty:
            return 0

def answer() -> i64:
    packet = Packet.Value(reference=1, type=2, __nagi_ident_0=3)
    return same_names(packet) + renamed(packet) + ignored(packet) + same_names(Packet.Empty)
"#;

#[test]
fn mutable_enum_patterns_preserve_names_and_compile_without_shorthand_warnings() {
    let mut high = parser::parse(SOURCE, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let high_rust = emit::rust(&checked_emission::seal(&high)).unwrap();
    assert_eq!(
        high_rust,
        emit::rust(&checked_emission::seal(&low)).unwrap()
    );

    let fixture = Fixture(std::env::temp_dir().join(format!(
        "nagi enum patterns {} {}",
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
        format!("{high_rust}\n#[test] fn patterns() {{ assert_eq!(answer(), 36); }}\n"),
    )
    .unwrap();
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args([
            "--edition=2021",
            "--test",
            "-D",
            "non_shorthand_field_patterns",
            "-D",
            "unused-imports",
        ])
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
