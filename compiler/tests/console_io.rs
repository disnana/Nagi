#[path = "support/native_triple.rs"]
mod native_triple;
use nagic::{check, emit, parser};
use native_triple::checked_emission;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

const HIGH: &str = "def input_with(prompt: view[str]) -> Result[str, Error]:\n    return read_line(prompt)\ndef main() -> Result[unit, Error]:\n    prompt = \"名前: \"\n    first = try read_line(prompt)\n    print(prompt)\n    second = try input_with(view(prompt))\n    third = try read_line()\n    print(first)\n    print(second)\n    return ok(print(third))\n";
const LOW: &str = "fn input_with(prompt: view[str]) -> Result[str, Error] { return read_line(prompt); }\nfn main() -> Result[unit, Error] { let prompt = \"名前: \"; let first = try read_line(prompt); print(prompt); let second = try input_with(view(prompt)); let third = try read_line(); print(first); print(second); return ok(print(third)); }\n";

#[test]
fn prompt_calls_preserve_high_saved_low_and_handwritten_low_emission() {
    let mut high = parser::parse(HIGH, true).unwrap();
    check::check(&mut high).unwrap();
    let saved = emit::low(&high);
    let mut low = parser::parse(&saved, false).unwrap();
    check::check(&mut low).unwrap();
    let high_rust = emit::rust(&checked_emission::seal(&high)).unwrap();
    assert_eq!(
        high_rust,
        emit::rust(&checked_emission::seal(&low)).unwrap()
    );
    assert_eq!(
        high_rust,
        emit::rust(&checked_emission::seal(&high)).unwrap()
    );
    assert!(high_rust.contains("::nagi_runtime::read_line()"));
    assert!(high_rust.contains("::nagi_runtime::read_line_prompt("));
    let mut handwritten = parser::parse(LOW, false).unwrap();
    check::check(&mut handwritten).unwrap();
    emit::rust(&checked_emission::seal(&handwritten)).unwrap();
}

#[test]
fn invalid_prompt_calls_are_checker_errors_at_the_original_call() {
    for (high_expr, low_expr, reason) in [
        (
            "return read_line(1)",
            "return read_line(1);",
            "promptはstrまたはview[str]",
        ),
        (
            "return read_line(True)",
            "return read_line(true);",
            "promptはstrまたはview[str]",
        ),
        (
            "return read_line([1])",
            "return read_line([1]);",
            "promptはstrまたはview[str]",
        ),
        (
            "return read_line(view([1]))",
            "return read_line(view([1]));",
            "promptはstrまたはview[str]",
        ),
        (
            "return read_line(\"x\", \"y\")",
            "return read_line(\"x\", \"y\");",
            "0または1引数",
        ),
        (
            "read_line(\"x\")\n    return ok(\"done\")",
            "read_line(\"x\"); return ok(\"done\");",
            "Resultを無視できません",
        ),
        (
            "return read_line[str](\"x\")",
            "return read_line[str](\"x\");",
            "型引数の数が一致しません",
        ),
    ] {
        let high = format!("def inspect() -> Result[str, Error]:\n    {high_expr}\n");
        let low = format!("fn inspect() -> Result[str, Error] {{\n    {low_expr}\n}}\n");
        let saved = emit::low(&parser::parse(&high, true).unwrap());
        for (source, high) in [(&high, true), (&low, false), (&saved, false)] {
            let mut parsed = parser::parse(source, high).expect("negative must reach checker");
            let call_line = parsed.functions[0].body[0].line;
            let error = check::check(&mut parsed).expect_err(source);
            assert!(
                error.starts_with(&format!("line {call_line}:")),
                "{source}: {error}"
            );
            assert!(error.contains(reason), "{source}: {error}");
        }
    }
    for source in [
        "def main():\n    print()\n",
        "def main():\n    print(1, 2)\n",
        "def main():\n    len()\n",
    ] {
        let mut parsed = parser::parse(source, true).unwrap();
        assert!(check::check(&mut parsed).unwrap_err().contains("引数"));
    }
    for (source, high) in [
        ("def inspect() -> str:\n    return read_line(\"x\")\n", true),
        ("fn inspect() -> str { return read_line(\"x\"); }\n", false),
    ] {
        let mut parsed = parser::parse(source, high).unwrap();
        assert!(check::check(&mut parsed)
            .unwrap_err()
            .contains("型が一致しません"));
    }
}

#[test]
fn real_console_input_runs_with_owned_and_borrowed_prompts_in_all_source_forms() {
    let fixture = native_triple::Fixture::new();
    fixture.write("main.nagi", HIGH);
    let high = fixture.checked("main.nagi").unwrap();
    fixture.write("saved.low", &emit::low(&high));
    fixture.write("handwritten.low", LOW);
    fixture.write("input.txt", "  A  \r\nB\nC\n");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("native-target"));
    for (name, profile) in [
        ("main.nagi", "full"),
        ("saved.low", "full"),
        ("handwritten.low", "full"),
        ("main.nagi", "auto"),
        ("saved.low", "auto"),
        ("handwritten.low", "auto"),
    ] {
        fixture.checked(name).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_nagic"));
        command
            .current_dir(&fixture.0)
            .args([
                "run",
                name,
                "--no-project",
                "--out",
                "out",
                "--profile",
                "dev",
                "--runtime-profile",
                profile,
            ])
            .env("NAGI_ROOT", &root)
            .env("NAGI_NATIVE_TARGET_DIR", &target)
            .env("CARGO_NET_OFFLINE", "true")
            .stdin(Stdio::from(
                fs::File::open(fixture.0.join("input.txt")).unwrap(),
            ));
        let output = fixture.run_bounded(&mut command, name, Duration::from_secs(180));
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            "名前: 名前: \n名前:   A  \nB\nC\n",
            "{name}"
        );
    }
}
