use nagic::{ast::Program, check, emit, parser};
use std::{fs, path::PathBuf, process::Command};

const INTEGER_TYPES: [&str; 8] = ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"];
const SIGNED_MINIMA: [(&str, &str); 4] = [
    ("i8", "-128"),
    ("i16", "-32768"),
    ("i32", "-2147483648"),
    ("i64", "-9223372036854775808"),
];

fn forms(high: &str, low: &str) -> [Program; 3] {
    let high = parser::parse(high, true).unwrap();
    // Preserve inferred/declared binding types in the actual High-to-Low
    // transport, even when the following domain validation rejects the AST.
    let mut annotated = high.clone();
    let _ = check::check(&mut annotated);
    let emitted = emit::low_with_lines(&annotated);
    let mut saved = parser::parse(&emitted.text, false).unwrap();
    emitted.restore_lines(&mut saved).unwrap();
    [high, parser::parse(low, false).unwrap(), saved]
}

fn rejects(high: &str, low: &str, code: &str, line: usize) {
    for mut program in forms(high, low) {
        let error = check::check(&mut program).expect_err(high);
        assert!(error.starts_with(&format!("line {line}:")), "{error}");
        assert!(error.contains(code), "{error}");
    }
}

#[test]
fn compound_and_alias_zero_fail_after_type_check_in_all_widths_and_forms() {
    for ty in INTEGER_TYPES {
        for op in ["/", "%"] {
            for zero in ["(7 - 7)", "(2 * 3 - 6)", "(9 % 3)", "(6 / 2 - 3)"] {
                rejects(
                    &format!("def value(input: {ty}) -> {ty}:\n    return input {op} {zero}\n"),
                    &format!(
                        "fn value(input: {ty}) -> {ty} {{\n    return input {op} {zero};\n}}\n"
                    ),
                    "E_CONST_ZERO_DIVISOR",
                    2,
                );
            }
            rejects(
                &format!("def value(input: {ty}) -> {ty}:\n    zero: {ty} = 2 - 2\n    return input {op} zero\n"),
                &format!("fn value(input: {ty}) -> {ty} {{\n    let zero: {ty} = 2 - 2;\n    return input {op} zero;\n}}\n"),
                "E_CONST_ZERO_DIVISOR",
                3,
            );
        }
    }
}

#[test]
fn signed_minimum_division_and_remainder_are_static_failures_in_every_width() {
    for (ty, minimum) in SIGNED_MINIMA {
        for op in ["/", "%"] {
            rejects(
                &format!("def value() -> {ty}:\n    return {minimum} {op} -1\n"),
                &format!("fn value() -> {ty} {{\n    return {minimum} {op} -1;\n}}\n"),
                "E_CONST_SIGNED_DIV_OVERFLOW",
                2,
            );
            rejects(
                &format!("def value() -> {ty}:\n    minimum: {ty} = {minimum}\n    negative: {ty} = -1\n    return minimum {op} negative\n"),
                &format!("fn value() -> {ty} {{\n    let minimum: {ty} = {minimum};\n    let negative: {ty} = -1;\n    return minimum {op} negative;\n}}\n"),
                "E_CONST_SIGNED_DIV_OVERFLOW",
                4,
            );
        }
    }
}

#[test]
fn compound_failures_are_checked_even_in_dead_branches_and_short_circuit_rhs() {
    for (high, low, line) in [
        ("def value() -> bool:\n    return False and (1 / (1 - 1) == 0)\n", "fn value() -> bool {\n    return false and (1 / (1 - 1) == 0);\n}\n", 2),
        ("def value() -> bool:\n    return True or (1 % (1 - 1) == 0)\n", "fn value() -> bool {\n    return true or (1 % (1 - 1) == 0);\n}\n", 2),
        ("def value() -> i64:\n    if False:\n        return 1 / (1 - 1)\n    return 2\n", "fn value() -> i64 {\n    if false {\n        return 1 / (1 - 1);\n    }\n    return 2;\n}\n", 3),
    ] {
        rejects(high, low, "E_CONST_ZERO_DIVISOR", line);
    }
}

#[test]
fn typed_ast_errors_precede_new_constant_diagnostics() {
    for (high, low, expected) in [
        (
            "def value() -> i64:\n    first = 1 / (1 - 1)\n    return missing\n",
            "fn value() -> i64 {\n    let first: i64 = 1 / (1 - 1);\n    return missing;\n}\n",
            "未定義の変数: missing",
        ),
        (
            "def value() -> u8:\n    return 1 / -(1 - 1)\n",
            "fn value() -> u8 {\n    return 1 / -(1 - 1);\n}\n",
            "u8は符号反転できません",
        ),
        (
            "def value() -> i64:\n    return \"text\" / (1 - 1)\n",
            "fn value() -> i64 {\n    return \"text\" / (1 - 1);\n}\n",
            "型が一致しません",
        ),
    ] {
        for mut program in forms(high, low) {
            let error = check::check(&mut program).unwrap_err();
            assert!(error.contains(expected), "{error}");
            assert!(!error.contains("E_CONST_"), "{error}");
        }
    }
}

#[test]
fn binding_facts_follow_reassignment_scope_and_continuing_branch_joins() {
    for (high, low, line) in [
        ("def value() -> i64:\n    divisor = 2\n    divisor = 0\n    return 1 / divisor\n", "fn value() -> i64 {\n    let divisor: i64 = 2;\n    divisor = 0;\n    return 1 / divisor;\n}\n", 4),
        ("def value(flag: bool) -> i64:\n    divisor = 2\n    if flag:\n        divisor = 0\n    else:\n        divisor = 0\n    return 1 / divisor\n", "fn value(flag: bool) -> i64 {\n    let divisor: i64 = 2;\n    if flag {\n        divisor = 0;\n    } else {\n        divisor = 0;\n    }\n    return 1 / divisor;\n}\n", 0),
        ("def value(flag: bool) -> i64:\n    divisor = 2\n    if flag:\n        return 3\n    else:\n        divisor = 0\n    return 1 / divisor\n", "fn value(flag: bool) -> i64 {\n    let divisor: i64 = 2;\n    if flag {\n        return 3;\n    } else {\n        divisor = 0;\n    }\n    return 1 / divisor;\n}\n", 0),
        ("async def value() -> Result[i64, Error]:\n    divisor = 2\n    scope:\n        divisor = 0\n    return ok(1 / divisor)\n", "async fn value() -> Result[i64, Error] {\n    let divisor: i64 = 2;\n    scope {\n        divisor = 0;\n    }\n    return ok(1 / divisor);\n}\n", 0),
    ] {
        for mut program in forms(high, low) {
            let error = check::check(&mut program).expect_err(high);
            assert!(error.contains("E_CONST_ZERO_DIVISOR"), "{error}");
            if line != 0 {
                assert!(error.starts_with(&format!("line {line}:")), "{error}");
            }
        }
    }
}

#[test]
fn loop_writes_kill_facts_before_conditions_and_respect_shadowed_counter_bindings() {
    for high in [
        "def value(flag: bool) -> i64:\n    divisor = 0\n    while flag and (1 / divisor == 0):\n        divisor = 2\n    return 1 / divisor\n",
        "def value() -> i64:\n    divisor = 0\n    for counter in range(2):\n        divisor = 2\n    return 1 / divisor\n",
        "def value() -> i64:\n    divisor = 2\n    for divisor in range(2):\n        divisor = 0\n    return 1 / divisor\n",
        "def value(flag: bool) -> i64:\n    divisor = 2\n    if flag:\n        divisor = 0\n    return 1 / divisor\n",
    ] {
        let mut high = parser::parse(high, true).unwrap();
        check::check(&mut high).unwrap();
        let mut saved = parser::parse(&emit::low(&high), false).unwrap();
        check::check(&mut saved).unwrap();
    }
}

#[test]
fn builtin_widening_uses_resolution_and_result_casts_remain_opaque() {
    rejects(
        "def value() -> i64:\n    zero: u8 = 0\n    return 1 / i64(zero)\n",
        "fn value() -> i64 {\n    let zero: u8 = 0;\n    return 1 / i64(zero);\n}\n",
        "E_CONST_ZERO_DIVISOR",
        3,
    );
    for source in [
        "def i64(value: i64) -> i64:\n    return value\ndef calculate() -> i64:\n    return 1 / i64(0)\n",
        "def value() -> Result[i32, Error]:\n    divisor = try i32(0)\n    return ok(1 / divisor)\n",
    ] {
        let mut high = parser::parse(source, true).unwrap();
        check::check(&mut high).unwrap();
        let mut saved = parser::parse(&emit::low(&high), false).unwrap();
        check::check(&mut saved).unwrap();
    }
}

#[test]
fn profile_dependent_intermediates_are_not_folded_to_a_known_zero() {
    for (ty, overflowing) in [
        ("i8", "127 + 1"),
        ("i16", "32767 + 1"),
        ("i32", "2147483647 + 1"),
        ("i64", "9223372036854775807 + 1"),
        ("u8", "255 + 1"),
        ("u16", "65535 + 1"),
        ("u32", "4294967295 + 1"),
        ("u64", "18446744073709551615 + 1"),
    ] {
        let high = format!(
            "def value() -> {ty}:\n    divisor: {ty} = {overflowing}\n    return 1 / divisor\n"
        );
        let low = format!("fn value() -> {ty} {{\n    let divisor: {ty} = {overflowing};\n    return 1 / divisor;\n}}\n");
        for mut program in forms(&high, &low) {
            check::check(&mut program).unwrap();
        }
    }
}

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn minimum_negation_and_integer_oracle_build_and_run_in_both_profiles() {
    let mut high = String::new();
    let mut oracle = String::new();
    for (ty, minimum) in SIGNED_MINIMA {
        high.push_str(&format!("def minimum_{ty}() -> {ty}:\n    return {minimum}\ndef negate_{ty}() -> {ty}:\n    return -({minimum})\n"));
        oracle.push_str(&format!("assert_eq!(minimum_{ty}(), {ty}::MIN); if cfg!(debug_assertions) {{ assert!(std::panic::catch_unwind(negate_{ty}).is_err()); }} else {{ assert_eq!(negate_{ty}(), {ty}::MIN); }}\n"));
    }
    for ty in INTEGER_TYPES {
        high.push_str(&format!(
            "def arithmetic_{ty}() -> {ty}:\n    return (7 * 3 + 3) / 2 % 7\n"
        ));
        oracle.push_str(&format!(
            "assert_eq!(arithmetic_{ty}(), (7{ty} * 3 + 3) / 2 % 7);\n"
        ));
    }
    for (ty, minimum, maximum) in [
        ("i8", "-128", "127"),
        ("i16", "-32768", "32767"),
        ("i32", "-2147483648", "2147483647"),
        ("i64", "-9223372036854775808", "9223372036854775807"),
        ("u8", "0", "255"),
        ("u16", "0", "65535"),
        ("u32", "0", "4294967295"),
        ("u64", "0", "18446744073709551615"),
    ] {
        for (operation, expression, expected) in [
            (
                "add",
                format!("{maximum} + 1"),
                format!("{ty}::MAX.wrapping_add(1)"),
            ),
            (
                "subtract",
                format!("{minimum} - 1"),
                format!("{ty}::MIN.wrapping_sub(1)"),
            ),
            (
                "multiply",
                format!("{maximum} * 2"),
                format!("{ty}::MAX.wrapping_mul(2)"),
            ),
        ] {
            high.push_str(&format!(
                "def overflow_{operation}_{ty}() -> {ty}:\n    return {expression}\n"
            ));
            oracle.push_str(&format!("if cfg!(debug_assertions) {{ assert!(std::panic::catch_unwind(overflow_{operation}_{ty}).is_err()); }} else {{ assert_eq!(overflow_{operation}_{ty}(), {expected}); }}\n"));
        }
    }
    high.push_str("def signed_quotient() -> i64:\n    return -7 / 3\ndef signed_remainder() -> i64:\n    return -7 % 3\ndef comparison() -> bool:\n    return not (7 * 2 < 10) and (3 + 4 == 7)\n");
    oracle.push_str("assert_eq!(signed_quotient(), -7i64 / 3); assert_eq!(signed_remainder(), -7i64 % 3); assert!(comparison());\n");
    let fixture = Fixture(std::env::temp_dir().join(format!(
            "nagi-constant-contract-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    fs::create_dir_all(&fixture.0).unwrap();
    let mut program = parser::parse(&high, true).unwrap();
    check::check(&mut program).unwrap();
    let mut saved = parser::parse(&emit::low(&program), false).unwrap();
    check::check(&mut saved).unwrap();
    for (form, program) in [program, saved].iter().enumerate() {
        let rust = emit::rust(program).unwrap();
        assert!(!rust.contains("allow(unconditional_panic)"));
        fs::write(
            fixture.0.join("generated.rs"),
            format!("{rust}\n#[test] fn contract() {{ {oracle} }}\n"),
        )
        .unwrap();
        for release in [false, true] {
            let binary = fixture.0.join(format!(
                "run-{form}-{release}{}",
                std::env::consts::EXE_SUFFIX
            ));
            let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
                .args([
                    "--edition=2021",
                    "--test",
                    "-C",
                    if release {
                        "overflow-checks=off"
                    } else {
                        "overflow-checks=on"
                    },
                    "-C",
                    if release {
                        "debug-assertions=off"
                    } else {
                        "debug-assertions=on"
                    },
                ])
                .arg(fixture.0.join("generated.rs"))
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
            assert!(output.status.success(), "{output:?}");
        }
    }
}

#[test]
fn compound_diagnostics_identify_original_modules_before_cargo() {
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "nagi-constant-diagnostics-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    )));
    fs::create_dir_all(&fixture.0).unwrap();
    for (extension, helper, entry) in [
        (
            "nagi",
            "def calculate() -> i64:\n    # 元の除数位置\n    return 1 / (1 - 1)\n",
            "import \"境界.nagi\" as boundary\ndef main():\n    print(42)\n",
        ),
        (
            "low",
            "fn calculate() -> i64 {\n    # 元の除数位置\n    return 1 / (1 - 1);\n}\n",
            "import \"境界.low\" as boundary;\nfn main() -> unit { print(42); }\n",
        ),
    ] {
        fs::write(fixture.0.join(format!("境界.{extension}")), helper).unwrap();
        let entry_path = format!("main.{extension}");
        fs::write(fixture.0.join(&entry_path), entry).unwrap();
        for action in ["check", "build", "run"] {
            let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
                .args([action, &entry_path, "--no-project"])
                .current_dir(&fixture.0)
                .env("PATH", "")
                .env("NAGI_ROOT", fixture.0.join("missing-runtime"))
                .output()
                .unwrap();
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "{output:?}");
            assert!(diagnostic.contains("E_CONST_ZERO_DIVISOR"), "{diagnostic}");
            assert!(
                diagnostic.contains(&format!("境界.{extension}:3")),
                "{diagnostic}"
            );
            assert!(diagnostic.contains("1 / (1 - 1)"), "{diagnostic}");
            assert!(!fixture.0.join("build").exists(), "{diagnostic}");
        }
    }
}
