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
