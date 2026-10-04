use nagic::{check, emit, parser};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

fn checked(text: &str, high: bool) -> Result<nagic::ast::Program, String> {
    let mut program = parser::parse(text, high)?;
    check::check(&mut program)?;
    Ok(program)
}

#[test]
fn direct_result_discard_rejects_outer_owned_wrappers_in_both_syntaxes() {
    for ty in [
        "Result[i64, Error]",
        "owned[Result[i64, Error]]",
        "owned[owned[Result[i64, Error]]]",
    ] {
        let high = format!("def discard(value: {ty}):\n    value\n");
        let low = format!("fn discard(value: {ty}) -> unit {{\n    value;\n}}\n");
        for (text, high) in [(&high, true), (&low, false)] {
            let error = checked(text, high).expect_err(text);
            assert!(error.starts_with("line 2:"), "{error}");
            assert!(error.contains("Resultを無視できません"), "{error}");
            if ty.starts_with("owned[") {
                assert!(error.contains("ownedで包んだ値"), "{error}");
                assert!(!error.contains("try"), "{error}");
            } else {
                assert!(error.contains("tryで伝播"), "{error}");
            }
        }
    }
}

#[test]
fn stored_passed_and_returned_owned_values_remain_accepted() {
    let high = checked(
        "class Packet:\n    name: str\ndef pass_result(value: owned[owned[Result[i64, Error]]]) -> owned[owned[Result[i64, Error]]]:\n    return value\ndef store_result(value: owned[owned[Result[i64, Error]]]) -> owned[owned[Result[i64, Error]]]:\n    saved = value\n    return pass_result(saved)\ndef pass_number(value: owned[i64]) -> owned[i64]:\n    saved = value\n    return saved\ndef pass_packet(value: owned[Packet]) -> owned[Packet]:\n    saved = value\n    return saved\ndef discard_number(value: owned[i64]):\n    value\ndef discard_packet(value: owned[Packet]):\n    value\ndef read_results(values: view[Result[i64, Error]]):\n    values\n    values\ndef discard_shared(value: shared[Result[i64, Error]]):\n    value\ndef discard_list(values: List[Result[i64, Error]]):\n    values\ndef main() -> Result[unit, Error]:\n    value = try parse_i64(\"42\")\n    match parse_i64(\"invalid\"):\n        case Ok(number):\n            print(number)\n        case Err(problem):\n            print(error_kind(problem))\n    return ok(print(value))\n",
        true,
    )
    .unwrap();
    let low = emit::low(&high);
    let reparsed = checked(&low, false).unwrap_or_else(|error| panic!("{low}\n{error}"));
    assert_eq!(emit::rust(&high).unwrap(), emit::rust(&reparsed).unwrap());
}

#[test]
fn wrapped_result_operations_and_type_identity_are_not_made_transparent() {
    for (text, reason) in [
        ("def propagate(value: owned[Result[i64, Error]]) -> Result[i64, Error]:\n    return ok(try value)\n", "try対象はResult"),
        ("def inspect(value: owned[Result[i64, Error]]):\n    match value:\n        case Ok(number):\n            print(number)\n        case Err(_):\n            print(0)\n", "matchの対象はResult"),
        ("def make() -> owned[Result[i64, Error]]:\n    return ok(1)\n", "型が一致しません"),
        ("def erase(value: owned[i64]) -> i64:\n    return value\n", "型が一致しません"),
        ("@rust(\"native::failing\")\nextern async def failing() -> owned[Result[unit, Error]]\nasync def main() -> Result[unit, Error]:\n    async with scope:\n        spawn failing()\n    return ok(print(0))\n", "spawnはasync unitまたはResult"),
    ] {
        let parsed = parser::parse(text, true).unwrap();
        let low = emit::low(&parsed);
        for (source, high) in [(text, true), (low.as_str(), false)] {
            let error = checked(source, high).expect_err(source);
            assert!(error.contains(reason), "{source}\n{error}");
        }
    }
}

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "nagi result discard {} {}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("adapter.rs"),
            "pub fn failing() -> Result<i64, nagi_runtime::Error> { Err(nagi_runtime::Error::invalid(\"probe failure\")) }\n",
        )
        .unwrap();
        Self(root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn extern_failures_are_rejected_before_backend_or_execution_in_both_syntaxes() {
    for ty in [
        "Result[i64, Error]",
        "owned[Result[i64, Error]]",
        "owned[owned[Result[i64, Error]]]",
    ] {
        let high = format!("@rust(\"native::failing\")\nextern def failing() -> {ty}\ndef main():\n    failing()\n    print(\"completed\")\n");
        let low = format!("@rust(\"native::failing\")\nextern fn failing() -> {ty};\nfn main() -> unit {{\n    failing();\n    print(\"completed\");\n}}\n");
        for (name, text) in [("main.nagi", high), ("main.low", low)] {
            let fixture = Fixture::new();
            fs::write(fixture.0.join(name), &text).unwrap();
            for command in ["check", "lower", "build", "run"] {
                let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
                    .current_dir(&fixture.0)
                    .args([
                        command,
                        name,
                        "--no-project",
                        "--rust",
                        "adapter.rs",
                        "--out",
                        "output",
                    ])
                    .env("PATH", "")
                    .env("NAGI_ROOT", fixture.0.join("missing-runtime"))
                    .output()
                    .unwrap();
                let error = String::from_utf8_lossy(&output.stderr);
                assert!(!output.status.success(), "{command} accepted {text}");
                assert!(error.contains(&format!("{name}:4")), "{error}");
                assert!(error.contains("Resultを無視できません"), "{error}");
                assert!(
                    !error.contains("Rust backend") && !error.contains("Cargo"),
                    "{error}"
                );
                assert!(output.stdout.is_empty(), "{output:?}");
                assert!(!fixture.0.join("output/src/main.rs").exists());
                assert!(!fixture.0.join("output/generated.low").exists());
            }
        }
    }
}
