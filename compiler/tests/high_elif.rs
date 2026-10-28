#[path = "support/checked_emission.rs"]
mod checked_emission;

use nagic::{ast::S, check, emit, parser};
use std::{fs, path::PathBuf, process::Command};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "nagi-high-elif-{}-{}",
                std::process::id(),
                ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("reserve fixture: {e}"),
            }
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const CHAIN: &str = "def classify(value: i64) -> i64:\n    if value < 0:\n        return -1\n    elif value == 0:\n        return 0\n    elif value < 10:\n        if value == 1:\n            return 11\n        elif value == 2:\n            return 12\n        else:\n            return 13\n    else:\n        return 20\n\ndef optional(value: i64) -> i64:\n    result = 30\n    if value == 0:\n        result = 31\n    elif value == 1:\n        result = 32\n    return result\n";

#[test]
fn elif_lowers_to_existing_if_statements_with_original_branch_lines() {
    let p = parser::parse(CHAIN, true).unwrap();
    let S::If(_, _, no) = &p.functions[0].body[0].kind else {
        panic!()
    };
    assert_eq!(no.len(), 1);
    assert_eq!(no[0].line, 4);
    let S::If(_, _, no) = &no[0].kind else {
        panic!()
    };
    assert_eq!(no[0].line, 6);
    let S::If(_, yes, no) = &no[0].kind else {
        panic!()
    };
    assert_eq!(no[0].line, 14);
    let S::If(_, _, nested) = &yes[0].kind else {
        panic!()
    };
    assert_eq!(nested[0].line, 9);
}

#[test]
fn chain_nested_optional_and_legacy_branches_execute_in_high_and_saved_low() {
    let mut source = format!("{CHAIN}\ndef legacy(value: i64) -> i64:\n    if value < 0:\n        return 40\n    else:\n        if value == 0:\n            return 41\n        else:\n            return 42\n\ndef tick(tag: i64, answer: bool) -> bool:\n    print(tag)\n    return answer\ndef ordered(first: bool) -> i64:\n    if tick(101, first):\n        return 1\n    elif tick(102, True):\n        return 2\n    elif tick(103, True):\n        return 3\n    return 4\n");
    source.push_str(include_str!("../../examples/tutorial/branches.nagi"));
    let mut high = parser::parse(&source, true).unwrap();
    check::check(&mut high).unwrap();
    let saved = emit::low_with_lines(&high);
    assert!(!saved.text.contains("elif"));
    let mut low = parser::parse(&saved.text, false).unwrap();
    check::check(&mut low).unwrap();
    let f = Fixture::new();
    for (form, program) in [high, low].iter().enumerate() {
        let generated = emit::rust(&checked_emission::seal(program)).unwrap();
        let rust = f.0.join("generated.rs");
        let binary =
            f.0.join(format!("run-{form}{}", std::env::consts::EXE_SUFFIX));
        fs::write(&rust, format!("{generated}\n#[test] fn branches() {{\nfor (v, expected) in [(-1,-1),(0,0),(1,11),(2,12),(9,13),(10,20)] {{ assert_eq!(classify(v), expected); }}\nfor (v, expected) in [(0,31),(1,32),(2,30)] {{ assert_eq!(optional(v), expected); }}\nfor (v, expected) in [(-1,40),(0,41),(1,42)] {{ assert_eq!(legacy(v), expected); }}\nassert_eq!(ordered(true), 1); assert_eq!(ordered(false), 2);\ndescribe(-1); describe(0); describe(1);\n}}\n")).unwrap();
        let built = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2021", "--test"])
            .arg(rust)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let ran = Command::new(binary).arg("--nocapture").output().unwrap();
        assert!(
            ran.status.success(),
            "{}",
            String::from_utf8_lossy(&ran.stdout)
        );
        let output = String::from_utf8(ran.stdout).unwrap();
        assert!(output.contains("101\n101\n102\n"), "{output}");
        assert!(!output.contains("103\n"), "{output}");
        assert!(output.contains("negative\nzero\npositive\n"), "{output}");
    }
}

#[test]
fn multiline_conditions_and_comments_preserve_clause_alignment() {
    let source = "def value(flag: bool) -> i64:\n    if flag:\n        return 1\n    # next condition\n\n    elif (\n        not flag\n    ): # explanation\n        return 2\n    else:\n        return 3\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
}

#[test]
fn returning_elif_paths_do_not_pollute_continuing_ownership_joins() {
    for (branch_return, accepted) in [("        return\n", true), ("", false)] {
        let source = format!("def take(value: str):\n    print(value)\ndef use(flag: i64):\n    text = \"hello\"\n    if flag == 0:\n        take(text)\n        return\n    elif flag == 1:\n        take(text)\n{branch_return}    print(text)\n");
        let mut high = parser::parse(&source, true).unwrap();
        let result = check::check(&mut high);
        assert_eq!(result.is_ok(), accepted, "{result:?}");
        let mut low = parser::parse(&emit::low(&high), false).unwrap();
        assert_eq!(check::check(&mut low).is_ok(), accepted);
        if let Err(error) = result {
            assert!(error.starts_with("line 10:"), "{error}");
            assert!(error.contains("move"), "{error}");
        }
    }
}

#[test]
fn elif_remains_an_identifier_in_high_and_low_even_after_if() {
    for (source, high) in [
        ("def elif(elif: i64) -> i64:\n    return elif + 1\ndef main():\n    if True:\n        print(0)\n    elif: i64 = 2\n    if False:\n        print(0)\n    elif += 1\n    print(elif)\n", true),
        ("def elif(value: i64):\n    print(value)\ndef main():\n    if True:\n        print(0)\n    elif(2)\n", true),
        ("def main():\n    if True:\n        elif: i64 = 2\n        print(elif)\n    if True:\n        elif : i64 = 3\n        print(elif)\n", true),
        ("def elif(value: i64):\n    print(value)\ndef main():\n    if True:\n        print(0)\n    elif(2); if True:\n        print(3)\n    elif = 4; if True:\n        print(elif)\n", true),
        ("fn elif(elif: i64) -> i64 { return elif + 1; }\nfn main() { if true { print(0); } let elif: i64 = 2; print(elif); }\n", false),
    ] {
        let mut p = parser::parse(source, high).unwrap();
        check::check(&mut p).unwrap();
        let mut low = parser::parse(&emit::low(&p), false).unwrap();
        check::check(&mut low).unwrap();
    }
}

#[test]
fn malformed_or_orphaned_clauses_fail_at_the_source_line() {
    for (source, high, line) in [
        ("def main():\n    elif True:\n        print(1)\n", true, 2),
        ("def main():\n    if True:\n        print(1)\n        elif False:\n            print(2)\n", true, 4),
        ("def main():\n    if True:\n        print(1)\n  elif False:\n        print(2)\n", true, 4),
        ("def main():\n    if True:\n        print(1)\n    elif:\n        print(2)\n", true, 4),
        ("def main():\n    if True:\n        print(1)\n    elif False\n        print(2)\n", true, 4),
        ("def main():\n    if True:\n        print(1)\n    elif False:\n    print(2)\n", true, 5),
        ("def main():\n    if True:\n        print(1)\n    else:\n        print(2)\n    elif False:\n        print(3)\n", true, 6),
        ("def main():\n    if True:\n        print(1)\n    else if False:\n        print(2)\n", true, 4),
        ("fn main() {\n    if true { print(1); }\n    elif false { print(2); }\n}\n", false, 3),
    ] {
        let error = parser::parse(source, high).unwrap_err();
        assert!(error.starts_with(&format!("line {line}:")), "{source}\n{error}");
    }
}

#[test]
fn condition_and_body_diagnostics_keep_elif_origins_through_low_transport() {
    for (source, line, message) in [
        ("def main():\n    if True:\n        print(1)\n    elif 2:\n        print(2)\n", 4, "bool"),
        ("def main():\n    if True:\n        print(1)\n    elif missing:\n        print(2)\n", 4, "missing"),
        ("def main():\n    if True:\n        print(1)\n    elif False:\n        print(missing)\n", 5, "missing"),
    ] {
        let high = parser::parse(source, true).unwrap();
        let generated = emit::low_with_lines(&high);
        let mut low = parser::parse(&generated.text, false).unwrap();
        generated.restore_lines(&mut low).unwrap();
        for mut p in [high, low] {
            let error = check::check(&mut p).unwrap_err();
            assert!(error.starts_with(&format!("line {line}:")), "{error}");
            assert!(error.contains(message), "{error}");
        }
    }
}

#[test]
fn elif_return_flow_requires_a_return_on_every_continuing_path() {
    for (source, accepted) in [
        ("def value(flag: bool) -> i64:\n    if flag:\n        return 1\n    elif not flag:\n        return 2\n    else:\n        return 3\n", true),
        ("def value(flag: bool) -> i64:\n    if flag:\n        return 1\n    elif not flag:\n        return 2\n", false),
    ] {
        let mut p = parser::parse(source, true).unwrap();
        let checked = check::check(&mut p);
        assert_eq!(checked.is_ok(), accepted, "{checked:?}");
        let mut saved = parser::parse(&emit::low(&p), false).unwrap();
        assert_eq!(check::check(&mut saved).is_ok(), accepted);
    }
}

#[test]
fn elif_chains_respect_the_lowered_block_nesting_budget() {
    let mut source = String::from("def main():\n    if False:\n        print(0)\n");
    for _ in 0..62 {
        source.push_str("    elif False:\n        print(0)\n");
    }
    let mut high = parser::parse(&source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    source.push_str("    elif False:\n        print(0)\n");
    let error = parser::parse(&source, true).unwrap_err();
    assert!(error.contains("64"), "{error}");
}
