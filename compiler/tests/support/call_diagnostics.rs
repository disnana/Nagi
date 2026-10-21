#[path = "checked_emission.rs"]
mod checked_emission;

use nagic::{ast::Program, check, emit, parser, source};
use std::{fs, path::PathBuf, process::Command};

fn forms(high: &str, low: &str) -> [Program; 3] {
    let high = parser::parse(high, true).expect("High fixture must parse");
    let mut annotated = high.clone();
    // Retain inferred local function types even when a later call is invalid.
    let _ = check::check(&mut annotated);
    let emitted = emit::low_with_lines(&annotated);
    let mut saved = parser::parse(&emitted.text, false).expect("saved Low must parse");
    emitted.restore_lines(&mut saved).unwrap();
    [
        high,
        parser::parse(low, false).expect("Low fixture must parse"),
        saved,
    ]
}

fn rejects(high: &str, low: &str, line: usize, prefix: &str, name: &str, counts: &str) {
    for mut program in forms(high, low) {
        let error = check::check(&mut program).expect_err("call must fail in checker");
        assert!(
            error.starts_with(&format!("line {line}: {prefix}")),
            "{error}"
        );
        assert!(error.contains(counts), "{error}");
        assert!(error.contains(name), "{error}");
        assert!(!error.contains("__nagi_def_"), "{error}");
    }
}

#[test]
fn direct_calls_report_expected_and_supplied_counts_including_zero() {
    for (params, call, counts) in [
        ("a: i64, b: i64", "1", "期待: 2個、実際: 1個"),
        ("a: i64", "1, 2", "期待: 1個、実際: 2個"),
        ("a: i64", "", "期待: 1個、実際: 0個"),
        ("", "1", "期待: 0個、実際: 1個"),
    ] {
        rejects(
            &format!("def calculate({params}):\n    print(42)\ndef main():\n    calculate({call})\n"),
            &format!("fn calculate({params}) -> unit {{ print(42); }}\nfn main() -> unit {{\n\n    calculate({call});\n}}\n"),
            4, "引数の数が一致しません", "calculate", counts,
        );
    }
}

#[test]
fn local_function_values_use_the_local_name_and_shadowed_signature() {
    rejects(
        "def pair(a: i64, b: i64):\n    print(42)\ndef main():\n    print = pair\n    print(1)\n",
        "fn pair(a: i64, b: i64) -> unit { print(42); }\nfn main() -> unit {\n\n    let print: fn[i64, i64, unit] = pair;\n    print(1);\n}\n",
        5, "引数の数が一致しません", "print", "期待: 2個、実際: 1個",
    );
    rejects(
        "def selected(a: i64):\n    print(a)\ndef empty():\n    print(42)\ndef main():\n    selected = empty\n    selected(1)\n",
        "fn selected(a: i64) -> unit { print(a); }\nfn empty() -> unit { print(42); }\nfn main() -> unit {\n\n\n    let selected: fn[unit] = empty;\n    selected(1);\n}\n",
        7, "引数の数が一致しません", "selected", "期待: 0個、実際: 1個",
    );
}

#[test]
fn async_direct_and_local_calls_report_counts() {
    for callee in ["answer", "selected"] {
        rejects(
            &format!("async def answer(value: i64) -> i64:\n    return value\nasync def main():\n    selected = answer\n    print(await {callee}())\n"),
            &format!("async fn answer(value: i64) -> i64 {{ return value; }}\nasync fn main() -> unit {{\n\n    let selected: fn[i64, Future[i64]] = answer;\n    print(await {callee}());\n}}\n"),
            5, "引数の数が一致しません", callee, "期待: 1個、実際: 0個",
        );
    }
}

#[test]
fn local_internal_looking_names_keep_the_user_spelling() {
    for mut program in forms(
        "def identity(value: i64) -> i64:\n    return value\ndef main():\n    __nagi_def_6d_f_66 = identity\n    __nagi_def_6d_f_66()\n",
        "fn identity(value: i64) -> i64 { return value; }\nfn main() -> unit {\n\n    let __nagi_def_6d_f_66: fn[i64, i64] = identity;\n    __nagi_def_6d_f_66();\n}\n",
    ) {
        assert_eq!(
            check::check(&mut program).unwrap_err(),
            "line 5: 引数の数が一致しません: __nagi_def_6d_f_66（期待: 1個、実際: 0個）"
        );
    }
}

#[test]
fn global_internal_looking_user_names_only_decode_after_module_resolution() {
    let high = "def __nagi_def_6d_f_66(value: i64) -> i64:\n    return value\ndef main():\n    # original call\n    __nagi_def_6d_f_66()\n";
    let low = "fn __nagi_def_6d_f_66(value: i64) -> i64 { return value; }\n\nfn main() -> unit {\n    # original call\n    __nagi_def_6d_f_66();\n}\n";
    let errors = forms(high, low).map(|mut program| check::check(&mut program).unwrap_err());
    let fixture = Fixture::new();
    for (name, text) in [("main.nagi", high), ("main.low", low)] {
        fs::write(fixture.0.join(name), text).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .args(["check", name, "--no-project"])
            .current_dir(&fixture.0)
            .output()
            .unwrap();
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{output:?}");
        assert!(error.contains(&format!("{name}:5")), "{error}");
        assert!(
            error.contains("__nagi_def_6d_f_66（期待: 1個、実際: 0個）"),
            "{error}"
        );
        assert!(!error.contains("m::f"), "{error}");
    }
    assert_eq!(
        errors,
        ["line 5: 引数の数が一致しません: __nagi_def_6d_f_66（期待: 1個、実際: 0個）"; 3]
    );
}

#[test]
fn checker_builtins_keep_their_prefix_and_add_supplied_counts() {
    for (call, prefix, name, counts) in [
        (
            "print()",
            "print は1引数です",
            "print",
            "期待: 1個、実際: 0個",
        ),
        (
            "clock_ns(1)",
            "clock_ns は0引数です",
            "clock_ns",
            "期待: 0個、実際: 1個",
        ),
    ] {
        rejects(
            &format!("def main():\n    {call}\n"),
            &format!("fn main() -> unit {{\n    {call};\n}}\n"),
            2,
            prefix,
            name,
            counts,
        );
    }
}

#[test]
fn builtin_type_arguments_report_name_and_expected_and_supplied_counts() {
    for (call, name, counts) in [
        ("print[i64](1)", "print", "期待: 0個、実際: 1個"),
        ("json_decode(\"42\")", "json_decode", "期待: 1個、実際: 0個"),
        ("size_of[i64, str]()", "size_of", "期待: 1個、実際: 2個"),
    ] {
        rejects(
            &format!("def main():\n    {call}\n"),
            &format!("fn main() -> unit {{\n    {call};\n}}\n"),
            2,
            "型引数の数が一致しません",
            name,
            counts,
        );
    }
}

#[test]
fn call_target_and_type_errors_keep_precedence() {
    for (high, low, line, reason) in [
        (
            "def main():\n    missing(1, 2)\n",
            "fn main() -> unit {\n    missing(1, 2);\n}\n",
            2,
            "未定義の関数: missing",
        ),
        (
            "def main():\n    print = 42\n    print()\n",
            "fn main() -> unit {\n    let print: i64 = 42;\n    print();\n}\n",
            3,
            "print は呼び出せる関数ではありません",
        ),
        (
            "def main():\n    missing[i64]()\n",
            "fn main() -> unit {\n    missing[i64]();\n}\n",
            2,
            "未定義の関数: missing",
        ),
        (
            "def main():\n    print = 42\n    print[i64]()\n",
            "fn main() -> unit {\n    let print: i64 = 42;\n    print[i64]();\n}\n",
            3,
            "print は呼び出せる関数ではありません",
        ),
        (
            "def main():\n    print[Missing]()\n",
            "fn main() -> unit {\n    print[Missing]();\n}\n",
            2,
            "未定義の型: Missing",
        ),
    ] {
        for mut program in forms(high, low) {
            assert_eq!(
                check::check(&mut program).unwrap_err(),
                format!("line {line}: {reason}")
            );
        }
    }
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "nagi-call-diagnostics-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create fixture: {error}"),
            }
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn imported_calls_keep_readable_names_and_original_source_positions_before_build() {
    let fixture = Fixture::new();
    for (extension, helper, entry) in [
        ("nagi", "def calculate(value: i64) -> i64:\n    return value\ndef invoke() -> i64:\n    # original call\n    return calculate()\n", "import \"helper.nagi\" as helper\ndef main():\n    print(helper.invoke())\n"),
        ("low", "fn calculate(value: i64) -> i64 { return value; }\n\nfn invoke() -> i64 {\n    # original call\n    return calculate();\n}\n", "import \"helper.low\" as helper;\nfn main() -> unit { print(helper.invoke()); }\n"),
    ] {
        fs::write(fixture.0.join(format!("helper.{extension}")), helper).unwrap();
        let entry_name = format!("main.{extension}");
        let entry_path = fixture.0.join(&entry_name);
        fs::write(&entry_path, entry).unwrap();
        let mut loaded = source::load(&entry_path, extension == "nagi").unwrap();
        let error = check::check(&mut loaded.program).unwrap_err();
        assert!(error.contains("引数の数が一致しません"), "{error}");
        assert!(error.contains("期待: 1個、実際: 0個"), "{error}");
        assert!(error.contains("calculate"), "{error}");
        assert!(!error.contains("__nagi_def_"), "{error}");
        let diagnostic = loaded.diagnostic(&error);
        assert!(diagnostic.contains(&format!("helper.{extension}:5")), "{diagnostic}");
        let emitted = emit::low_with_lines(&loaded.program);
        let mut saved = parser::parse(&emitted.text, false).unwrap();
        emitted.restore_lines(&mut saved).unwrap();
        assert_eq!(check::check(&mut saved).unwrap_err(), error);
        for action in ["check", "build", "run"] {
            let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
                .args([action, &entry_name, "--no-project"])
                .current_dir(&fixture.0)
                .env("PATH", "")
                .env("NAGI_ROOT", fixture.0.join("missing-runtime"))
                .output().unwrap();
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "{output:?}");
            assert!(error.contains("期待: 1個、実際: 0個"), "{error}");
            assert!(error.contains(&format!("helper.{extension}:5")), "{error}");
            assert!(!error.contains("__nagi_def_"), "{error}");
            assert!(!fixture.0.join("build").exists(), "{error}");
        }
    }
}

#[test]
fn valid_neighbor_calls_still_check_emit_build_and_run() {
    let fixture = Fixture::new();
    let high = "def add(a: i64, b: i64) -> i64:\n    return a + b\ndef empty() -> i64:\n    return 40\ndef answer() -> i64:\n    selected = add\n    return selected(empty(), len([40, 2]))\nasync def async_answer(value: i64) -> i64:\n    return value\nasync def async_alias() -> i64:\n    selected = async_answer\n    return await selected(42)\n";
    let low = "fn add(a: i64, b: i64) -> i64 { return a + b; }\nfn empty() -> i64 { return 40; }\nfn answer() -> i64 { let selected: fn[i64, i64, i64] = add; return selected(empty(), len([40, 2])); }\nasync fn async_answer(value: i64) -> i64 { return value; }\nasync fn async_alias() -> i64 { let selected: fn[i64, Future[i64]] = async_answer; return await selected(42); }\n";
    for (index, mut program) in forms(high, low).into_iter().enumerate() {
        check::check(&mut program).unwrap();
        let rust = emit::rust(&checked_emission::seal(&program)).unwrap();
        let path = fixture.0.join(format!("valid-{index}.rs"));
        let oracle = r#"
#[test]
fn calls() {
    assert_eq!(answer(), 42);
    struct Noop;
    impl std::task::Wake for Noop {
        fn wake(self: std::sync::Arc<Self>) {}
    }
    let waker = std::task::Waker::from(std::sync::Arc::new(Noop));
    let mut context = std::task::Context::from_waker(&waker);
    let mut future = Box::pin(async_alias());
    assert_eq!(std::future::Future::poll(future.as_mut(), &mut context), std::task::Poll::Ready(42));
}
"#;
        fs::write(&path, format!("{rust}\n{oracle}")).unwrap();
        let binary = fixture
            .0
            .join(format!("valid-{index}{}", std::env::consts::EXE_SUFFIX));
        let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2021", "--test"])
            .arg(&path)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(binary).output().unwrap();
        assert!(output.status.success(), "{output:?}");
    }
}
