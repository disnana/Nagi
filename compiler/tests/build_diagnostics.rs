use nagic::{check, diagnostics, emit, parser, source};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

static FIXTURE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi diagnostics 凪 {} {} {}",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        let f = Self(path);
        // These fixtures use only std intrinsics. A dependency-free runtime
        // keeps actual Cargo/CLI coverage fast and independent of downloads.
        f.write(
            "runtime/Cargo.toml",
            "[package]\nname='nagi-runtime'\nversion='0.1.0'\nedition='2021'\n[workspace]\n",
        );
        f.write("runtime/src/lib.rs", "");
        f
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(&self.0)
            .args(args)
            .env("NAGI_ROOT", &self.0)
            .env("NAGI_NATIVE_TARGET_DIR", self.0.join("target"))
            .env("CARGO_NET_OFFLINE", "true")
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const HIGH_MOVE: &str = "def take(text: str):\n    print(text)\n\ndef main():\n    name = \"凪\"\n    for number in range(2):\n        take(name)\n";
const LOW_MOVE: &str = "fn take(text: str) -> unit { print(text); }\n\nfn main() -> unit {\n    let name: str = \"凪\";\n    for number in range(2) {\n        take(name);\n    }\n}\n";

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn mapped_prefix(text: &str) -> &str {
    text.split(" note: Rust backend details (generated code):")
        .next()
        .unwrap()
}

#[test]
fn high_build_points_to_nagi_and_keeps_rust_notes_and_failure_status() {
    let f = Fixture::new();
    f.write("main.nagi", HIGH_MOVE);
    assert!(f.cli(&["check", "main.nagi"]).status.success());
    let output = f.cli(&["build", "main.nagi"]);
    assert!(!output.status.success());
    let text = stderr(&output);
    let prefix = mapped_prefix(&text);
    assert!(prefix.contains("error[E0382]"), "{text}");
    assert!(prefix.contains("main.nagi:7"), "{text}");
    assert!(prefix.contains("7 |         take(name)"), "{text}");
    assert!(prefix.contains("main.nagi:5"), "{text}");
    assert!(!prefix.contains("src/main.rs:"), "{text}");
    assert!(text.contains("name.clone()"), "{text}");
    assert!(text.contains("Rust backend rejected program"), "{text}");
}

#[test]
fn imported_high_uses_the_dependency_path_and_local_line() {
    let f = Fixture::new();
    f.write(
        "good.nagi",
        &format!("{}def good():\n    print(1)\n", "\n".repeat(40)),
    );
    f.write(
        "lib/move.nagi",
        &HIGH_MOVE.replace("def main():", "def repeat():"),
    );
    f.write(
        "main.nagi",
        "import \"good.nagi\"\nimport \"lib/move.nagi\"\ndef main():\n    repeat()\n",
    );
    let output = f.cli(&["build", "main.nagi"]);
    assert!(!output.status.success());
    let text = stderr(&output);
    let prefix = mapped_prefix(&text);
    assert!(prefix.contains("move.nagi:7"), "{text}");
    assert!(prefix.contains("7 |         take(name)"), "{text}");
    assert!(!prefix.contains("main.nagi:"), "{text}");
}

#[test]
fn standalone_low_uses_the_original_low_lines() {
    let f = Fixture::new();
    f.write("main.low", LOW_MOVE);
    let output = f.cli(&["build", "main.low"]);
    assert!(!output.status.success());
    let text = stderr(&output);
    assert!(mapped_prefix(&text).contains("main.low:6"), "{text}");
    assert!(
        mapped_prefix(&text).contains("6 |         take(name);"),
        "{text}"
    );
}

#[test]
fn imported_low_uses_its_own_file_instead_of_the_entry_file() {
    let f = Fixture::new();
    f.write(
        "lib/move.low",
        &LOW_MOVE.replace("fn main()", "fn repeat()"),
    );
    f.write(
        "main.low",
        "import \"lib/move.low\";\nfn main() -> unit { repeat(); }\n",
    );
    let output = f.cli(&["build", "main.low"]);
    assert!(!output.status.success());
    let text = stderr(&output);
    assert!(mapped_prefix(&text).contains("move.low:6"), "{text}");
    assert!(!mapped_prefix(&text).contains("main.low:"), "{text}");
}

#[test]
fn replacement_body_errors_point_to_the_handwritten_low_file() {
    let f = Fixture::new();
    f.write(
        "main.nagi",
        "def repeat():\n    print(1)\ndef main():\n    repeat()\n",
    );
    f.write(
        "native/replace.low",
        &format!(
            "@replace generated::repeat\n{}",
            LOW_MOVE.replace("fn main()", "fn replace_body()")
        ),
    );
    let output = f.cli(&["build", "main.nagi", "--native", "native/replace.low"]);
    assert!(!output.status.success());
    let text = stderr(&output);
    assert!(mapped_prefix(&text).contains("replace.low:7"), "{text}");
    assert!(!mapped_prefix(&text).contains("main.nagi:"), "{text}");
}

#[test]
fn native_signature_errors_also_keep_the_native_low_location() {
    let f = Fixture::new();
    f.write("main.nagi", "def value() -> i64:\n    return 1\n");
    f.write(
        "native.low",
        "@replace generated::value\nfn replacement() -> str { return \"wrong\"; }\n",
    );
    let output = f.cli(&["check", "main.nagi", "--native", "native.low"]);
    assert!(!output.status.success());
    let text = stderr(&output);
    assert!(text.contains("native.low:2"), "{text}");
    assert!(text.contains("fn replacement()"), "{text}");
}

#[test]
fn native_rust_errors_are_not_relabelled_as_nagi() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    broken()\n");
    f.write(
        "bridge.low",
        "@rust(\"native::broken\")\nextern fn broken() -> unit;\n",
    );
    f.write(
        "bridge.rs",
        "pub fn broken() {\n    let value: String = 12;\n}\n",
    );
    let output = f.cli(&[
        "build",
        "main.nagi",
        "--native",
        "bridge.low",
        "--rust",
        "bridge.rs",
    ]);
    assert!(!output.status.success());
    let text = stderr(&output);
    assert!(text.contains("bridge.rs:2:"), "{text}");
    assert!(text.contains("mismatched types"), "{text}");
    assert!(!text.contains("main.nagi:"), "{text}");
    assert!(!text.contains("Rust backend details"), "{text}");
}

#[test]
fn a_successful_run_stays_quiet_and_passes_through_program_output() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(\"凪\")\n");
    let output = f.cli(&["run", "main.nagi"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("native: "), "{stdout}");
    assert!(stdout.ends_with("凪\n"), "{stdout}");
    assert!(!stdout.contains("compiler-artifact"), "{stdout}");
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(!text.contains("warning:"), "{text}");
    assert!(!text.contains("Rust backend details"), "{text}");
}

#[test]
fn lowering_restores_nested_statement_expression_arm_and_field_lines() {
    let f = Fixture::new();
    let text = "class Model:\n    value: i64\n\ndef nested() -> i64:\n    total = 0\n    if true:\n        while false:\n            total = 1\n    else:\n        for number in range(2):\n            total = number\n    match parse_i64(\"1\"):\n        case Ok(value):\n            return value\n        case Err(_):\n            return total\n";
    f.write("lib/nested.nagi", text);
    f.write(
        "main.nagi",
        "import \"lib/nested.nagi\"\ndef main():\n    print(nested())\n",
    );
    let mut loaded = source::load(&f.0.join("main.nagi"), true).unwrap();
    check::check(&mut loaded.program).unwrap();
    let low = emit::low_with_lines(&loaded.program);
    let mut restored = parser::parse(&low.text, false).unwrap();
    low.restore_lines(&mut restored).unwrap();
    check::check(&mut restored).unwrap();
    let rust = emit::rust_with_lines(&restored).unwrap();
    for (fragment, local) in [
        ("pub value: i64", 2),
        ("while false", 7),
        ("total = 1", 8),
        ("for number", 10),
        ("total = number", 11),
        ("Ok(mut value)", 13),
        ("return value", 14),
        ("Err(_)", 15),
        ("return total", 16),
    ] {
        let line = rust
            .text
            .lines()
            .position(|s| s.contains(fragment))
            .unwrap()
            + 1;
        let location = loaded.location(rust.line_origin(line).unwrap()).unwrap();
        assert_eq!(location.line, local, "{fragment}");
        assert!(location.path.ends_with("lib/nested.nagi"), "{fragment}");
    }
    let field = restored.classes[0].field_lines[0];
    assert_eq!(loaded.location(field).unwrap().line, 2);
}

fn message(file: &Path, span_file: &str, line: usize, rendered: &str) -> Value {
    json!({
        "reason": "compiler-message",
        "target": {"src_path": file},
        "message": {
            "level": "error",
            "message": "fixture diagnostic",
            "code": {"code": "E0382"},
            "rendered": rendered,
            "spans": [{
                "file_name": span_file, "line_start": line,
                "is_primary": true, "label": "value moved here"
            }]
        }
    })
}

#[test]
fn windows_paths_and_canonical_prefixes_match_only_the_generated_target() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(1)\n");
    let mut loaded = source::load(&f.0.join("main.nagi"), true).unwrap();
    check::check(&mut loaded.program).unwrap();
    let rust = emit::rust_with_lines(&loaded.program).unwrap();
    let line = rust
        .text
        .lines()
        .position(|s| s.contains("println!"))
        .unwrap()
        + 1;
    let file = Path::new(r"\\?\C:\nagi project\build\src\main.rs");
    for span_file in [
        "src/main.rs",
        r"src\main.rs",
        r"C:\nagi project\build\src\main.rs",
    ] {
        let mut value = message(file, span_file, line, "original Rust details\n");
        value["target"]["src_path"] = json!(r"C:\nagi project\build\src\main.rs");
        let text = diagnostics::cargo_message(&value.to_string(), &rust, file, &loaded).unwrap();
        assert!(mapped_prefix(&text).contains("main.nagi:2"), "{text}");
    }
}

#[test]
fn dependency_synthetic_and_incomplete_diagnostics_fall_back_without_guessing() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(1)\n");
    let mut loaded = source::load(&f.0.join("main.nagi"), true).unwrap();
    check::check(&mut loaded.program).unwrap();
    let rust = emit::rust_with_lines(&loaded.program).unwrap();
    let file = f.0.join("build/src/main.rs");
    let fallback = "original warning and suggestions\n";
    let mut dependency = message(&file, "src/main.rs", 2, fallback);
    dependency["target"]["src_path"] = json!(f.0.join("dependency/src/main.rs"));
    let synthetic = message(&file, "src/main.rs", 1, fallback);
    let mut missing = message(&file, "src/main.rs", 2, fallback);
    missing["message"]["spans"] = Value::Null;
    for value in [dependency, synthetic, missing] {
        assert_eq!(
            diagnostics::cargo_message(&value.to_string(), &rust, &file, &loaded).unwrap(),
            fallback
        );
    }
    let plain = "unexpected Cargo output";
    assert_eq!(
        diagnostics::cargo_message(plain, &rust, &file, &loaded).unwrap(),
        format!("{plain}\n")
    );
    assert!(diagnostics::cargo_message(
        r#"{"reason":"build-finished","success":false}"#,
        &rust,
        &file,
        &loaded
    )
    .is_none());
}
