use nagic::{check, emit, parser};
use std::{fs, path::PathBuf, process::Command};

static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "nagi-frontend-{}-{}",
            std::process::id(),
            ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    fn write(&self, name: &str, source: &str) {
        fs::write(self.0.join(name), source).unwrap();
    }

    fn run(&self, command: &str, name: &str) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_nagic"))
            .args([command, name, "--no-project", "--out", "out"])
            .current_dir(&self.0)
            .output()
            .unwrap()
    }

    fn succeeds(&self, command: &str, name: &str) -> std::process::Output {
        let output = self.run(command, name);
        assert!(
            output.status.success(),
            "{command} {name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn roundtrip(source: &str) -> String {
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let generated = emit::low(&high);
    let mut low = parser::parse(&generated, false).unwrap();
    check::check(&mut low).unwrap();
    generated
}

#[test]
fn forward_invalid_result_signature_is_a_checker_diagnostic() {
    let high = "def caller() -> Result[i64, Error]:\n    return ok(try later())\ndef later() -> Result[i64]:\n    return 1\n";
    let handwritten = "fn caller() -> Result[i64, Error] { return ok(try later()); }\nfn later() -> Result[i64] { return 1; }\n";
    let saved = emit::low(&parser::parse(high, true).expect("valid High syntax"));
    let fixture = Fixture::new();
    for (name, text, high) in [
        ("bad.nagi", high, true),
        ("saved.low", saved.as_str(), false),
        ("handwritten.low", handwritten, false),
    ] {
        let mut program = parser::parse(text, high).expect("syntax must reach the checker");
        let declaration = program.functions[1].line;
        let checked =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check::check(&mut program)));
        let error = checked
            .expect("invalid signature must not panic")
            .unwrap_err();
        assert!(
            error.starts_with(&format!("line {declaration}:")),
            "{error}"
        );
        assert!(error.contains("型引数は2個です"), "{error}");
        fixture.write(name, text);
        let output = fixture.run("check", name);
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{diagnostic}");
        assert!(diagnostic.contains("型引数は2個です"), "{diagnostic}");
        assert!(
            diagnostic.contains(&format!("{name}:{declaration}")),
            "{diagnostic}"
        );
        assert!(!diagnostic.contains("panicked"), "{diagnostic}");
    }
}

#[test]
fn flat_left_associative_expression_roundtrips_without_added_depth() {
    // A shallow source expression, just beyond the former generated nesting
    // limit. Its value is runtime-dependent, so constant errors cannot mask it.
    let expression = format!("value{}", " - 1".repeat(129));
    let high = format!("def answer(value: i64) -> i64:\n    return {expression}\ndef main():\n    print(answer(0))\n");
    let handwritten = format!("fn answer(value: i64) -> i64 {{ return {expression}; }}\nfn main() {{ print(answer(0)); }}\n");
    let fixture = Fixture::new();
    fixture.write("main.nagi", &high);
    fixture.succeeds("check", "main.nagi");
    let saved = fs::read_to_string(fixture.0.join("out/generated.low"))
        .expect("successful High check must preserve generated Low");
    fixture.write("saved.low", &saved);
    fixture.write("handwritten.low", &handwritten);
    for name in ["main.nagi", "saved.low", "handwritten.low"] {
        fixture.succeeds("check", name);
        let output = fixture.succeeds("run", name);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).lines().last(),
            Some("-129")
        );
    }
}

#[test]
fn generated_low_preserves_right_grouping_and_prefix_precedence() {
    let generated = roundtrip(
        "def answer(value: i64) -> i64:\n    return - (value - (5 - 2))\ndef main():\n    print(answer(20))\n",
    );
    assert!(
        generated.contains("return - (value - (5 - 2));"),
        "{generated}"
    );
}

#[test]
fn combined_generated_low_does_not_use_the_source_file_byte_limit() {
    let fixture = Fixture::new();
    // Two retained literals cross the combined artifact boundary with only a
    // few AST nodes; source, nesting, import-count, and work limits are separate.
    let left = format!(
        "def left() -> str:\n    return \"{}\"\n",
        "a".repeat(1_000_000)
    );
    let right = format!(
        "def right() -> str:\n    return \"{}\"\n",
        "b".repeat(1_000_000)
    );
    let main = "import \"left.nagi\"\nimport \"right.nagi\"\ndef main():\n    a = left()\n    b = right()\n    print(len(view(a)) + len(view(b)))\n";
    assert!(left.len() < 2_000_000 && right.len() < 2_000_000);
    assert!(left.len() + right.len() + main.len() < 8_000_000);
    fixture.write("left.nagi", &left);
    fixture.write("right.nagi", &right);
    fixture.write("main.nagi", main);
    let mut loaded = nagic::source::load(&fixture.0.join("main.nagi"), true)
        .expect("all source budgets pass before lowering");
    check::check(&mut loaded.program).expect("High semantics pass before lowering");
    assert!(emit::low(&loaded.program).len() > 2_000_000);
    fixture.succeeds("check", "main.nagi");
    let output = fixture.succeeds("run", "main.nagi");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).lines().last(),
        Some("2000000")
    );
}

#[test]
fn source_file_byte_limit_still_applies_to_high_and_handwritten_low() {
    let oversized = format!("#{}", "x".repeat(2_000_000));
    for high in [true, false] {
        assert_eq!(
            parser::parse(&oversized, high).unwrap_err(),
            "source limit: 2 MB"
        );
    }
}

#[test]
fn index_lookahead_does_not_accumulate_nesting_across_statements_or_functions() {
    let mut source = String::from("def main():\n    values = [7]\n");
    for _ in 0..80 {
        source.push_str("    print(values[0])\n    print(values[[0][0]])\n");
    }
    source.push_str("    value: i64 = 2\ndef next(value: i64) -> i64:\n    return value\n");
    let generated = roundtrip(&source);
    let f = Fixture::new();
    f.write("main.nagi", &source);
    f.write("main.low", &generated);
    for name in ["main.nagi", "main.low"] {
        f.succeeds("check", name);
        f.succeeds("lower", name);
        let output = f.succeeds("symbols", name);
        let index: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(index["locals"]
            .as_array()
            .unwrap()
            .iter()
            .any(|local| local["name"] == "value" && local["type"] == "i64"));
    }
}

#[test]
fn speculative_index_recovery_preserves_actual_nesting_limits() {
    for high in [true, false] {
        let expression = format!("{}1", "- ".repeat(140));
        let source = if high {
            format!("def main():\n    print({expression})\n")
        } else {
            format!("fn main() {{ print({expression}); }}\n")
        };
        assert!(parser::parse(&source, high)
            .unwrap_err()
            .contains("式の入れ子が深すぎます"));

        let ty = format!("{}i64{}", "List[".repeat(65), "]".repeat(65));
        let source = if high {
            format!("def nested(value: {ty}):\n    print(0)\n")
        } else {
            format!("fn nested(value: {ty}) {{ print(0); }}\n")
        };
        assert!(parser::parse(&source, high)
            .unwrap_err()
            .contains("型の入れ子は64段までです"));
    }
}

#[test]
fn lowering_keeps_try_and_await_as_field_and_index_receivers() {
    let source = "class Point:\n    x: i64\n\
        def point() -> Result[Point, Error]:\n    return ok(Point(x=7))\n\
        def numbers() -> Result[List[i64], Error]:\n    return ok([7])\n\
        async def async_point() -> Point:\n    return Point(x=7)\n\
        async def async_numbers() -> List[i64]:\n    return [7]\n\
        async def async_result() -> Result[Point, Error]:\n    return ok(Point(x=7))\n\
        def read_point() -> Result[i64, Error]:\n    return ok((try point()).x)\n\
        def read_numbers() -> Result[i64, Error]:\n    return ok((try numbers())[0])\n\
        async def await_point() -> i64:\n    return (await async_point()).x\n\
        async def await_numbers() -> i64:\n    return (await async_numbers())[0]\n\
        async def both() -> Result[i64, Error]:\n    return ok((try await async_result()).x)\n\
        def main():\n    print(0)\n";
    let generated = roundtrip(source);
    let f = Fixture::new();
    f.write("main.nagi", source);
    f.write("main.low", &generated);
    for name in ["main.nagi", "main.low"] {
        f.succeeds("check", name);
        f.succeeds("lower", name);
        let output = f.succeeds("symbols", name);
        let index: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(index["expressions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|expression| expression["type"] == "Point"
                && expression["fields"][0]["name"] == "x"));
    }
}

#[test]
fn record_constructors_reject_type_arguments_in_high_low_and_symbols() {
    let f = Fixture::new();
    for ty in ["Missing", "i64", "Point", "List[i64]"] {
        for (name, high) in [("main.nagi", true), ("main.low", false)] {
            let source = if high {
                format!("class Point:\n    x: i64\ndef main():\n    point = Point[{ty}](x=7)\n")
            } else {
                format!(
                    "record Point {{ x: i64; }}\nfn main() {{ let point = Point[{ty}](x=7); }}\n"
                )
            };
            assert!(parser::parse(&source, high)
                .unwrap_err()
                .contains("classの型引数は未対応です"));
            f.write(name, &source);
            for command in ["check", "lower", "symbols"] {
                let output = f.run(command, name);
                assert!(!output.status.success(), "{command} {name} accepted {ty}");
                assert!(
                    String::from_utf8_lossy(&output.stderr).contains("classの型引数は未対応です")
                );
            }
        }
    }
}

#[test]
fn generic_builtins_and_ordinary_record_constructors_still_roundtrip() {
    roundtrip("class Point:\n    x: i64\ndef decode() -> Result[Point, Error]:\n    return json_decode[Point](\"{}\")\ndef main():\n    point = Point(x=7)\n    print(point.x)\n");
}
