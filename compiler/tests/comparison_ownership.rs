#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, parser, source};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi-comparison-ownership-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn checked(text: &str, high: bool) -> Result<nagic::ast::Program, String> {
    let mut program = parser::parse(text, high)?;
    check::check(&mut program)?;
    Ok(program)
}

#[test]
fn comparisons_hold_the_left_operand_through_right_operand_evaluation() {
    for operator in ["==", "!=", "<", ">", "<=", ">="] {
        for left in ["text", "view(text)", "identity(view(text))"] {
            for (high, text, line) in [
                (
                    true,
                    format!("def consume(text: str) -> str:\n    return text\ndef identity(part: view[str]) -> view[str]:\n    return part\ndef main():\n    text = \"Nagi\"\n    print({left} {operator} consume(text))\n"),
                    7,
                ),
                (
                    false,
                    format!("fn consume(text: str) -> str {{ return text; }}\nfn identity(part: view[str]) -> view[str] {{ return part; }}\nfn main() -> unit {{\n    let text: str = \"Nagi\";\n    print({left} {operator} consume(text));\n}}\n"),
                    5,
                ),
            ] {
                let error = checked(&text, high).expect_err(&text);
                assert!(error.starts_with(&format!("line {line}:")), "{error}");
                assert!(error.contains("同じ式で先に参照"), "{error}");
            }
        }
    }
}

#[test]
fn native_resource_comparisons_do_not_allow_moving_the_borrowed_request() {
    let fixture = Fixture::new();
    for (name, high, text, line) in [
        (
            "request.nagi",
            true,
            "import std.http.server as http\ndef take(request: http.Request) -> http.Method:\n    return request.method\ndef compare(request: http.Request) -> bool:\n    return request.method == take(request)\n",
            5,
        ),
        (
            "request.low",
            false,
            "import std.http.server as http;\nfn take(request: http.Request) -> http.Method { return request.method; }\nfn compare(request: http.Request) -> bool {\n    return request.method == take(request);\n}\n",
            4,
        ),
    ] {
        let path = fixture.0.join(name);
        fs::write(&path, text).unwrap();
        let mut loaded = source::load(&path, high).unwrap();
        let error = check::check(&mut loaded.program).expect_err(text);
        assert!(error.starts_with(&format!("line {line}:")), "{error}");
        assert!(error.contains("同じ式で先に参照"), "{error}");
    }
    for (name, high, text) in [
        (
            "getter.nagi",
            true,
            "import std.http.server as http\ndef take(response: http.Response) -> http.Status:\n    return response.status\ndef compare(response: http.Response) -> bool:\n    return response.status == take(response)\n",
        ),
        (
            "getter.low",
            false,
            "import std.http.server as http;\nfn take(response: http.Response) -> http.Status { return response.status; }\nfn compare(response: http.Response) -> bool { return response.status == take(response); }\n",
        ),
    ] {
        let path = fixture.0.join(name);
        fs::write(&path, text).unwrap();
        let mut loaded = source::load(&path, high).unwrap();
        check::check(&mut loaded.program).unwrap_or_else(|error| panic!("{text}\n{error}"));
    }
}

#[test]
fn copy_aggregate_comparisons_borrow_fields_and_indexed_containers() {
    for ty in [
        "Option[i64]",
        "owned[Option[i64]]",
        "UUID",
        "timestamp",
        "unit",
        "owned[unit]",
    ] {
        for (high, text, line) in [
            (
                true,
                format!("class Pair:\n    number: {ty}\n    text: str\ndef take(pair: Pair) -> {ty}:\n    return pair.number\ndef compare(pair: Pair) -> bool:\n    return pair.number == take(pair)\n"),
                7,
            ),
            (
                false,
                format!("record Pair {{ number: {ty}; text: str; }}\nfn take(pair: Pair) -> {ty} {{ return pair.number; }}\nfn compare(pair: Pair) -> bool {{\n    return pair.number == take(pair);\n}}\n"),
                4,
            ),
        ] {
            let error = checked(&text, high).expect_err(&text);
            assert!(error.starts_with(&format!("line {line}:")), "{error}");
            assert!(error.contains("同じ式で先に参照"), "{error}");
        }
    }

    for left in ["values[0]", "view(values)[0]"] {
        for (high, text) in [
            (true, format!("def take(values: List[Option[i64]]) -> Option[i64]:\n    return values[0]\ndef compare(values: List[Option[i64]]) -> bool:\n    return {left} == take(values)\n")),
            (false, format!("fn take(values: List[Option[i64]]) -> Option[i64] {{ return values[0]; }}\nfn compare(values: List[Option[i64]]) -> bool {{ return {left} == take(values); }}\n")),
        ] {
            let error = checked(&text, high).expect_err(&text);
            assert!(error.contains("同じ式で先に参照"), "{error}");
        }
    }
    for (high, text) in [
        (true, "class Number:\n    value: Option[i64]\ndef take(values: List[Number]) -> Option[i64]:\n    return values[0].value\ndef compare(values: List[Number]) -> bool:\n    return values[0].value == take(values)\n"),
        (false, "record Number { value: Option[i64]; }\nfn take(values: List[Number]) -> Option[i64] { return values[0].value; }\nfn compare(values: List[Number]) -> bool { return values[0].value == take(values); }\n"),
        (true, "class i64:\n    flag: bool\nclass Pair:\n    number: i64\n    text: str\ndef take(pair: Pair) -> i64:\n    return pair.number\ndef compare(pair: Pair) -> bool:\n    return pair.number == take(pair)\n"),
    ] {
        let error = checked(text, high).expect_err(text);
        assert!(error.contains("同じ式で先に参照"), "{error}");
    }
}

#[test]
fn completed_copy_operands_disjoint_fields_and_temporary_views_remain_valid() {
    let text = "class Pair:\n    number: i64\n    text: str\n    other: str\ndef take(pair: Pair) -> i64:\n    return pair.number\ndef consume(text: str) -> str:\n    return text\ndef disjoint(pair: Pair) -> bool:\n    return view(pair.text) == consume(pair.other)\ndef main():\n    pair = Pair(number=1, text=\"Nagi\", other=\"Nagi\")\n    print(pair.number == take(pair))\n";
    let high = checked(text, true).unwrap();
    let low = emit::low(&high);
    checked(&low, false).unwrap_or_else(|error| panic!("{low}\n{error}"));

    let text = "class Pair:\n    number: owned[owned[i64]]\n    text: str\ndef take(pair: Pair) -> owned[owned[i64]]:\n    return pair.number\ndef compare(pair: Pair) -> bool:\n    return pair.number == take(pair)\n";
    let high = checked(text, true).unwrap();
    let low = emit::low(&high);
    checked(&low, false).unwrap_or_else(|error| panic!("{low}\n{error}"));

    let text = "def consume(text: str) -> str:\n    return text\ndef length(text: str) -> i64:\n    return len(text)\ndef number(value: i64) -> i64:\n    return value\ndef take_values(values: List[i64]) -> i64:\n    return values[0]\ndef value() -> i64:\n    return 4\ndef take_functions(values: List[fn[i64]]) -> fn[i64]:\n    return values[0]\ndef main():\n    text = \"Nagi\"\n    other = \"Nagi\"\n    print(view(text) == consume(other))\n    print(view(text) == consume(copy(view(text))))\n    print(copy(view(text)) == consume(text))\n    text = \"Nagi\"\n    print(len(view(text)) == length(text))\n    number_value = 4\n    print(number_value == number(number_value))\n    values = [4]\n    print(values[0] == take_values(values))\n    functions = [value]\n    print(functions[0] == take_functions(functions))\n    print(view(\"Nagi\") == \"Nagi\")\n    text = \"Nagi\"\n    print(view(text) == \"Nagi\")\n    print(consume(text))\n";
    let high = checked(text, true).unwrap();
    let low = emit::low(&high);
    let low = checked(&low, false).unwrap();
    let rust = emit::rust(&checked_emission::seal(&high)).unwrap();
    assert_eq!(rust, emit::rust(&checked_emission::seal(&low)).unwrap());

    let fixture = Fixture::new();
    let source = fixture.0.join("main.rs");
    let executable = fixture
        .0
        .join(format!("main{}", std::env::consts::EXE_SUFFIX));
    fs::write(&source, rust).unwrap();
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2021", "-D", "unused-imports"])
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(executable).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .collect::<Vec<_>>(),
        ["true", "true", "true", "true", "true", "true", "true", "true", "true", "Nagi"]
    );
}
