use nagic::{ast::E, check, emit, parser};
use std::{fs, path::PathBuf, process::Command};

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn strings_preserve_control_characters_through_high_low_and_rust() {
    let expected = "start\0\x08\x0c\x1b\u{85}\u{2028}日本語😀\n\r\t\"\\end";
    let source = "def text() -> str:\n    return \"start\0\x08\x0c\x1b\u{85}\u{2028}日本語😀\\n\\r\\t\\\"\\\\end\"\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let generated = emit::low(&high);
    let mut low = parser::parse(&generated, false).unwrap();
    check::check(&mut low).unwrap();
    let fixture = Fixture(
        std::env::temp_dir().join(format!("nagi-literal-contracts-{}", std::process::id())),
    );
    fs::create_dir_all(&fixture.0).unwrap();
    for program in [high, low] {
        let nagic::ast::S::Return(Some(expr)) = &program.functions[0].body[0].kind else {
            panic!("expected string return");
        };
        assert!(matches!(&expr.kind, E::Str(value) if value == expected));
        let mut rust = emit::rust(&program).unwrap();
        rust.push_str(&format!(
            "\n#[test] fn exact_bytes() {{ assert_eq!(text().as_bytes(), {expected:?}.as_bytes()); }}\n"
        ));
        let source = fixture.0.join("generated.rs");
        let binary = fixture
            .0
            .join(format!("generated{}", std::env::consts::EXE_SUFFIX));
        fs::write(&source, rust).unwrap();
        let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2021", "--test", "-D", "warnings"])
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
        let output = Command::new(&binary).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}
