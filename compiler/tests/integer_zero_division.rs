use nagic::{ast::Program, check, emit, parser};
use std::{fs, path::PathBuf, process::Command};

const INTEGER_TYPES: [&str; 8] = ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"];

fn forms(high: &str, low: &str) -> [Program; 3] {
    let high = parser::parse(high, true).unwrap();
    // Serialize before checking so an invalid High expression can also be
    // tested through the independent saved-Low parser.
    let saved = parser::parse(&emit::low(&high), false).unwrap();
    [high, parser::parse(low, false).unwrap(), saved]
}

#[test]
fn explicit_integer_zero_divisors_fail_in_high_handwritten_low_and_saved_low() {
    for ty in INTEGER_TYPES {
        for op in ["/", "%"] {
            for numerator in ["1", "value"] {
                let mut zeros = vec!["0", "00", "(0)", "((0))"];
                if ty.starts_with('i') {
                    zeros.extend(["-0", "(-0)", "-(0)", "-(-0)"]);
                }
                for zero in zeros {
                    let high = format!(
                        "def calculate(value: {ty}) -> {ty}:\n    return {numerator} {op} {zero}\n"
                    );
                    let low = format!(
                        "fn calculate(value: {ty}) -> {ty} {{\n    return {numerator} {op} {zero};\n}}\n"
                    );
                    for mut program in forms(&high, &low) {
                        let error = check::check(&mut program).expect_err(&high);
                        assert!(error.contains(&format!("整数の{op}の除数に0")), "{error}");
                        assert!(error.contains("ゼロ以外の値"), "{error}");
                    }
                }
            }
        }
    }
}

#[test]
fn operand_and_earlier_type_errors_keep_priority_over_zero_divisor_errors() {
    for (high, low, expected) in [
        (
            "def value() -> i64:\n    return missing / 0\n",
            "fn value() -> i64 {\n    return missing / 0;\n}\n",
            "未定義の変数: missing",
        ),
        (
            "def value() -> i64:\n    return \"text\" / 0\n",
            "fn value() -> i64 {\n    return \"text\" / 0;\n}\n",
            "型が一致しません: expected str, got i64",
        ),
        (
            "def value() -> u8:\n    return 1 / -0\n",
            "fn value() -> u8 {\n    return 1 / -0;\n}\n",
            "u8は符号反転できません",
        ),
        (
            "def value() -> f64:\n    return 1.0 / 0\n",
            "fn value() -> f64 {\n    return 1.0 / 0;\n}\n",
            "型が一致しません: expected f64, got i64",
        ),
        (
            "def value() -> i64:\n    let wrong: i64 = \"text\"\n    return 1 / 0\n",
            "fn value() -> i64 {\n    let wrong: i64 = \"text\";\n    return 1 / 0;\n}\n",
            "型が一致しません: expected i64, got str",
        ),
    ] {
        for mut program in forms(high, low) {
            let error = check::check(&mut program).unwrap_err();
            assert!(error.contains(expected), "{error}");
            assert!(!error.contains("除数に0"), "{error}");
        }
    }
}

#[test]
fn check_does_not_evaluate_aliases_constant_expressions_or_signed_overflow() {
    let high = "def value() -> i64:\n    zero = 0\n    first = 1 / zero\n    second = 1 % (1 - 1)\n    third = -9223372036854775808 / -1\n    return first + second + third\n";
    let low = "fn value() -> i64 {\n    let zero: i64 = 0;\n    let first: i64 = 1 / zero;\n    let second: i64 = 1 % (1 - 1);\n    let third: i64 = -9223372036854775808 / -1;\n    return first + second + third;\n}\n";
    for mut program in forms(high, low) {
        check::check(&mut program).unwrap();
    }
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi integer zero 凪 {} {}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, file: &str, text: &str) {
        fs::write(self.0.join(file), text).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cli_rejects_zero_at_the_original_file_and_line_before_cargo() {
    for op in ["/", "%"] {
        let f = Fixture::new();
        let high = format!("def calculate() -> i64:\n    return 1 {op} 0\n");
        let low = format!("fn calculate() -> i64 {{\n    return 1 {op} 0;\n}}\n");
        f.write("main.nagi", &high);
        f.write("main.low", &low);
        let saved = emit::low(&parser::parse(&high, true).unwrap());
        f.write("saved.low", &saved);
        f.write(
            "entry.nagi",
            "import \"helper.nagi\"\ndef main():\n    print(42)\n",
        );
        f.write("helper.nagi", &high);
        f.write(
            "entry.low",
            "import \"helper.low\";\nfn main() -> unit { print(42); }\n",
        );
        f.write("helper.low", &low);
        let saved_line = saved
            .lines()
            .position(|line| line.contains("return"))
            .unwrap()
            + 1;
        for (file, location, line) in [
            ("main.nagi", "main.nagi", 2),
            ("main.low", "main.low", 2),
            ("saved.low", "saved.low", saved_line),
            ("entry.nagi", "helper.nagi", 2),
            ("entry.low", "helper.low", 2),
        ] {
            for action in ["check", "build", "run"] {
                let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
                    .args([action, file])
                    .current_dir(&f.0)
                    .env("PATH", "")
                    .env("NAGI_ROOT", f.0.join("missing-runtime"))
                    .output()
                    .unwrap();
                let error = String::from_utf8_lossy(&output.stderr);
                assert!(!output.status.success(), "{output:?}");
                assert!(error.contains(&format!("整数の{op}の除数に0")), "{error}");
                assert!(error.contains(&format!("{location}:{line}")), "{error}");
                assert!(error.contains(&format!("1 {op} 0")), "{error}");
                assert!(!f.0.join("build").exists(), "{output:?}");
            }
        }
    }
}

#[test]
fn dynamic_zero_nonzero_and_float_controls_compile_and_run_in_both_profiles() {
    let mut high = String::new();
    let mut low = String::new();
    let mut assertions = String::new();
    for ty in INTEGER_TYPES {
        for (name, op, result) in [("divide", "/", 3), ("remainder", "%", 1)] {
            let function = format!("{name}_{ty}");
            high.push_str(&format!("def {function}(value: {ty}, divisor: {ty}) -> {ty}:\n    return value {op} divisor\n"));
            low.push_str(&format!("fn {function}(value: {ty}, divisor: {ty}) -> {ty} {{ return value {op} divisor; }}\n"));
            assertions.push_str(&format!("assert_eq!({function}(7, 2), {result}); assert!(std::panic::catch_unwind(|| {function}(7, 0)).is_err());\n"));
            high.push_str(&format!(
                "def literal_{function}(value: {ty}) -> {ty}:\n    return value {op} 2\n"
            ));
            low.push_str(&format!(
                "fn literal_{function}(value: {ty}) -> {ty} {{ return value {op} 2; }}\n"
            ));
            assertions.push_str(&format!("assert_eq!(literal_{function}(7), {result});\n"));
        }
    }
    for ty in ["f32", "f64"] {
        for (name, expression, assertion) in [
            ("infinity", "1.0 / 0.0", ".is_infinite()"),
            ("negative_infinity", "1.0 / -0.0", ".is_sign_negative()"),
            ("nan", "0.0 / 0.0", ".is_nan()"),
            ("remainder", "1.0 % 0.0", ".is_nan()"),
        ] {
            let function = format!("float_{name}_{ty}");
            high.push_str(&format!(
                "def {function}() -> {ty}:\n    return {expression}\n"
            ));
            low.push_str(&format!(
                "fn {function}() -> {ty} {{ return {expression}; }}\n"
            ));
            assertions.push_str(&format!("assert!({function}(){assertion});\n"));
        }
    }
    let f = Fixture::new();
    for (index, mut program) in forms(&high, &low).into_iter().enumerate() {
        check::check(&mut program).unwrap();
        let rust = emit::rust(&program).unwrap();
        assert!(!rust.contains("allow(unconditional_panic)"));
        f.write(
            "generated.rs",
            &format!("{rust}\n#[test] fn controls() {{\n{assertions}}}\n"),
        );
        for release in [false, true] {
            let binary = f.0.join(format!(
                "controls-{index}-{release}{}",
                std::env::consts::EXE_SUFFIX
            ));
            let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
                .args(["--edition=2021", "--test", "-C"])
                .arg(if release {
                    "opt-level=3"
                } else {
                    "opt-level=0"
                })
                .args([
                    "-C",
                    if release {
                        "overflow-checks=off"
                    } else {
                        "overflow-checks=on"
                    },
                ])
                .arg(f.0.join("generated.rs"))
                .arg("-o")
                .arg(&binary)
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
            let output = Command::new(&binary).output().unwrap();
            assert!(output.status.success(), "{output:?}");
        }
    }
}
