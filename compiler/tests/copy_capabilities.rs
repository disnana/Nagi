#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, parser, source};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi-copy-capabilities-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn compile(&self, program: &nagic::ast::Program, name: &str, clone: bool) -> Output {
        let adapter = self.0.join(format!("{name}-native.rs"));
        fs::write(
            &adapter,
            format!("{}{}", if clone { CLONE } else { "" }, CONSUME),
        )
        .unwrap();
        // Include the adapter in the same native module used by --rust,
        // without involving Cargo or the runtime's dependency graph.
        let rust = format!(
            "{}\n#[path = {}]\nmod native;\n",
            emit::rust(&checked_emission::seal(program)).unwrap(),
            serde_json::to_string(&adapter.to_string_lossy()).unwrap()
        );
        let source = self.0.join(format!("{name}.rs"));
        fs::write(&source, rust).unwrap();
        Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2021", "-O"])
            .arg(source)
            .arg("-o")
            .arg(self.binary(name))
            .output()
            .unwrap()
    }

    fn binary(&self, name: &str) -> PathBuf {
        self.0
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn manual_clone_adapter_copies_owned_enum_payloads_in_high_and_independent_low() {
    let fixture = Fixture::new();
    let path = fixture.0.join("main.nagi");
    fs::write(&path, HIGH).unwrap();
    let mut loaded = source::load(&path, true).unwrap();
    check::check(&mut loaded.program).unwrap();
    let mut low = parser::parse(&emit::low(&loaded.program), false).unwrap();
    check::check(&mut low).unwrap();

    for (name, program) in [("high", &loaded.program), ("low", &low)] {
        let compiled = fixture.compile(program, name, true);
        assert!(
            compiled.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let output = Command::new(fixture.binary(name)).output().unwrap();
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        // Both independently owned lists survive until the Rust consumer
        // takes them; non-Copy does not mean that Clone is unavailable.
        assert_eq!(
            String::from_utf8(output.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            "Nagi\nNagi\n"
        );
    }
}

#[test]
fn omitted_clone_adapter_reports_the_element_clone_bound() {
    let fixture = Fixture::new();
    // This declaration is valid with the adapter above. Remove only its Rust
    // Clone implementation when compiling, rather than making acceptance of
    // an unsupported source program a permanent frontend contract.
    let mut high = parser::parse(HIGH, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();

    for (name, program) in [("high-missing", &high), ("low-missing", &low)] {
        let compiled = fixture.compile(program, name, false);
        assert!(
            !compiled.status.success(),
            "{name} accepted a missing Clone"
        );
        let error = String::from_utf8_lossy(&compiled.stderr);
        assert!(
            error.contains("Clone") && error.contains("Message"),
            "{error}"
        );
    }
}

const HIGH: &str = r#"enum Message:
    Text(value: str)
@rust("native::consume")
extern def consume(messages: List[Message]) -> str
def main():
    messages = [Message.Text("Nagi")]
    duplicate = copy(view(messages))
    print(consume(duplicate))
    print(consume(messages))
"#;

const CLONE: &str = r#"impl Clone for super::Message {
    fn clone(&self) -> Self {
        match self {
            Self::Text { value } => Self::Text { value: value.clone() },
        }
    }
}
"#;

const CONSUME: &str = r#"pub fn consume(messages: Vec<super::Message>) -> String {
    match messages.into_iter().next().unwrap() {
        super::Message::Text { value } => value,
    }
}
"#;
