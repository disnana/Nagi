use nagic::{check, emit, parser};
use std::{fs, path::PathBuf, process::Command};

static FIXTURE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn nested_views_compile_and_copy_strings_bytes_and_lists() {
    let mut high = parser::parse(
        "def text_copy(data: str) -> str:\n    return copy(view(data))\n\
         def bytes_copy(data: bytes) -> bytes:\n    return copy(view(data))\n\
         def list_copy(data: List[i64]) -> List[i64]:\n    return copy(view(data))\n\
         def text_len(data: str) -> i64:\n    return len(view(data))\n",
        true,
    )
    .unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let mut code = emit::rust(&low).unwrap();
    // These functions only use std, so the emitted code needs no runtime crate.
    code.push_str(
        "\n#[test] fn generated_values() {\n\
         assert_eq!(text_copy(String::from(\"Nagi\")), \"Nagi\");\n\
         assert_eq!(bytes_copy(vec![0, 128, 255]), vec![0, 128, 255]);\n\
         assert_eq!(list_copy(vec![1, 2, 3]), vec![1, 2, 3]);\n\
         assert_eq!(text_len(String::from(\"あ\")), 3);\n}\n",
    );
    compile_and_run(code);
}

fn compile_and_run(code: String) {
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "nagi-codegen-{}-{}-{}",
        std::process::id(),
            FIXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )));
    fs::create_dir(&fixture.0).unwrap();
    let source = fixture.0.join("generated.rs");
    let binary = fixture
        .0
        .join(format!("generated{}", std::env::consts::EXE_SUFFIX));
    fs::write(&source, code).unwrap();
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2021", "--test", "-D", "unused-imports"])
        .arg(&source)
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
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn result_match_compiles_and_runs_owned_and_borrowed_payloads() {
    let source = r#"def choose(r: Result[i64, i64]) -> i64:
    match r:
        case Ok(number):
            number += 1
            return number
        case Err(reason):
            return reason

def payload(r: Result[str, i64]) -> str:
    match r:
        case Ok(text):
            return text
        case Err(_):
            return "fallback"

def borrowed(r: Result[view[str], i64]) -> str:
    match r:
        case Ok(part):
            return copy(part)
        case Err(_):
            return "fallback"
"#;
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let mut code = emit::rust(&low).unwrap();
    code.push_str(
        r#"
#[test] fn generated_matches() {
    assert_eq!(choose(Ok(41)), 42);
    assert_eq!(choose(Err(-1)), -1);
    assert_eq!(payload(Ok(String::from("owned"))), "owned");
    assert_eq!(payload(Err(1)), "fallback");
    let text = String::from("borrowed");
    assert_eq!(borrowed(Ok(text.as_str())), "borrowed");
    assert_eq!(borrowed(Err(1)), "fallback");
}
"#,
    );
    compile_and_run(code);
}

#[test]
fn user_functions_with_builtin_names_keep_their_call_targets() {
    let source = "def len(values: view[i64]) -> i64:\n    return 99\ndef print(number: i64) -> i64:\n    return number + 1\ndef answer() -> i64:\n    values = [1, 2]\n    return print(len(view(values)))\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let mut code = emit::rust(&low).unwrap();
    code.push_str("\n#[test] fn calls_user_functions() { assert_eq!(answer(), 100); }\n");
    compile_and_run(code);
}

#[test]
fn user_and_local_functions_named_serve_compile_without_http_runtime() {
    for source in [
        "def serve(value: i64, port: i64) -> i64:\n    return value + port\ndef answer() -> i64:\n    return serve(40, 2)\n",
        "def add(value: i64, port: i64) -> i64:\n    return value + port\ndef answer() -> i64:\n    serve = add\n    return serve(40, 2)\n",
    ] {
        let mut high = parser::parse(source, true).unwrap();
        check::check(&mut high).unwrap();
        let mut low = parser::parse(&emit::low(&high), false).unwrap();
        check::check(&mut low).unwrap();
        for program in [&high, &low] {
            let mut code = emit::rust(program).unwrap();
            code.push_str("\n#[test] fn calls_serve() { assert_eq!(answer(), 42); }\n");
            compile_and_run(code);
        }
    }
}

#[test]
fn rust_keywords_and_generated_helper_names_remain_valid_nagi_names() {
    let source = "def type(self: i64, crate: i64, super: i64, Self: i64, __nagi_ident_0: i64) -> i64:\n    loop: i64 = self + crate + super + Self + __nagi_ident_0\n    return loop\ndef __nagi_main() -> i64:\n    return type(1, 2, 3, 4, 5)\ndef answer() -> i64:\n    return __nagi_main()\ndef main():\n    assert_true(answer() == 15)\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let mut code = emit::rust(&low).unwrap();
    code.push_str("\n#[test] fn calls_escaped_names() { assert_eq!(answer(), 15); main(); }\n");
    compile_and_run(code);
}

#[test]
fn local_function_values_shadow_builtins_and_functions() {
    let source = "def fixed(values: view[i64]) -> i64:\n    return 99\ndef fallback(values: view[i64]) -> i64:\n    return 7\ndef answer() -> i64:\n    len = fixed\n    fallback = len\n    values = [1, 2]\n    return len(view(values)) + fallback(view(values))\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let mut code = emit::rust(&low).unwrap();
    code.push_str("\n#[test] fn local_calls() { assert_eq!(answer(), 198); }\n");
    compile_and_run(code);
    let mut invalid = parser::parse("def main():\n    len = 1\n    len([1, 2])\n", true).unwrap();
    assert!(check::check(&mut invalid)
        .unwrap_err()
        .contains("呼び出せる関数"));
}

#[test]
fn builtin_option_and_result_constructors_do_not_call_user_functions() {
    let source = "def Some(value: i64) -> i64:\n    return 99\ndef Ok(value: i64) -> i64:\n    return 99\ndef option_value() -> i64?:\n    return some(42)\ndef result_value() -> Result[i64, i64]:\n    return ok(42)\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let mut code = emit::rust(&low).unwrap();
    code.push_str("\n#[test] fn intrinsic_constructors() { assert_eq!(option_value(), Some(42)); assert_eq!(result_value(), Ok(42)); }\n");
    // The test's expected values also use fully qualified Rust constructors.
    code = code
        .replace(
            "option_value(), Some(42)",
            "option_value(), ::std::option::Option::Some(42)",
        )
        .replace(
            "result_value(), Ok(42)",
            "result_value(), ::std::result::Result::Ok(42)",
        );
    compile_and_run(code);
}

#[test]
fn async_function_values_keep_their_future_return_type() {
    let source = "async def type(value: i64) -> i64:\n    return value + 1\nasync def answer() -> i64:\n    len = type\n    return await len(41)\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let mut code = emit::rust(&low).unwrap();
    code.push_str(r#"
#[test] fn async_alias() {
    let future = answer();
    let mut future = std::pin::pin!(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert_eq!(std::future::Future::poll(future.as_mut(), &mut context), std::task::Poll::Ready(42));
}
"#);
    compile_and_run(code);
}

#[test]
fn a_record_named_fn_does_not_become_a_copy_function_value() {
    let source = "class fn:\n    text: str\ndef take(value: fn):\n    print(value.text)\ndef main():\n    value = fn(text=\"Nagi\")\n    take(value)\n    take(value)\n";
    let mut p = parser::parse(source, true).unwrap();
    assert!(check::check(&mut p).unwrap_err().contains("move後"));
}

#[test]
fn function_values_preserve_the_shared_view_lifetime() {
    let source = "def select(first: view[i64], second: view[i64]) -> view[i64]:\n    return second\ndef answer() -> i64:\n    selected = select\n    first = [1]\n    second = [2, 3]\n    return len(selected(view(first), view(second)))\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let mut code = emit::rust(&low).unwrap();
    code.push_str("\n#[test] fn borrowed_alias() { assert_eq!(answer(), 2); }\n");
    compile_and_run(code);
}

#[test]
fn returning_a_function_value_does_not_borrow_local_data() {
    let source = "def identity(values: view[i64]) -> view[i64]:\n    return values\ndef provide() -> fn[view[i64], view[i64]]:\n    return identity\ndef answer() -> i64:\n    selected = provide()\n    values = [1, 2]\n    return len(selected(view(values)))\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let mut code = emit::rust(&low).unwrap();
    code.push_str("\n#[test] fn returned_function() { assert_eq!(answer(), 2); }\n");
    compile_and_run(code);
}

#[test]
fn output_compiles_for_numbers_booleans_and_borrowed_strings() {
    let source = r#"def output(text: str):
    a: i8 = -1
    b: i16 = -2
    c: i32 = -3
    d: i64 = -4
    e: u8 = 1
    f: u16 = 2
    g: u32 = 3
    h: u64 = 4
    i: f32 = 1.5
    j: f64 = 2.5
    print(a)
    print(b)
    print(c)
    print(d)
    print(e)
    print(f)
    print(g)
    print(h)
    print(i)
    print(j)
    print(True)
    write(False)
    print(text)
    write(view(text))
    print(text)
"#;
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let mut code = emit::rust(&low).unwrap();
    code.push_str("\n#[test] fn generated_output() { output(String::from(\"凪\")); }\n");
    compile_and_run(code);
}

#[test]
fn numeric_negation_and_nested_view_comparisons_compile_and_run() {
    let mut source = String::new();
    for ty in ["i8", "i16", "i32", "i64", "f32", "f64"] {
        source.push_str(&format!(
            "def negative_{ty}(value: {ty}) -> {ty}:\n    return -value\n"
        ));
    }
    for (name, ty) in [
        ("strings", "str"),
        ("bytes", "bytes"),
        ("lists", "List[i64]"),
        ("optional", "Option[i64]"),
        ("results", "Result[i64, i64]"),
        ("shared", "shared[i64]"),
        ("owned", "owned[i64]"),
    ] {
        source.push_str(&format!("def compare_{name}(values: view[{ty}]) -> bool:\n    return values == values and values <= values\n"));
    }
    source.push_str(
        "def equal_maps(values: view[Map[i64, str]]) -> bool:\n    return values == values\n",
    );
    let mut high = parser::parse(&source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let mut code = emit::rust(&low).unwrap();
    code.push_str(
        r#"
#[test] fn generated_operators() {
    assert_eq!(negative_i8(1), -1);
    assert_eq!(negative_i16(2), -2);
    assert_eq!(negative_i32(3), -3);
    assert_eq!(negative_i64(4), -4);
    assert_eq!(negative_f32(1.5), -1.5);
    assert_eq!(negative_f64(2.5), -2.5);
    assert!(compare_strings("凪"));
    assert!(compare_bytes(&[0, 128, 255]));
    assert!(compare_lists(&[vec![1, 2], vec![3]]));
    assert!(compare_optional(&[None, Some(42)]));
    assert!(compare_results(&[Ok(42), Err(1)]));
    assert!(compare_shared(&[std::sync::Arc::new(42)]));
    assert!(compare_owned(&[42]));
    assert!(equal_maps(&[std::collections::HashMap::from([(42, String::from("Nagi"))])]));
}
"#,
    );
    compile_and_run(code);
}

#[test]
fn explicit_local_async_function_annotations_compile_and_run() {
    let source = "async def increment(value: i64) -> i64:\n    return value + 1\nasync def answer() -> i64:\n    selected: fn[i64, Future[i64]] = increment\n    return await selected(41)\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let mut code = emit::rust(&low).unwrap();
    code.push_str(r#"
#[test] fn explicit_async_alias() {
    let mut future = std::pin::pin!(answer());
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert_eq!(std::future::Future::poll(future.as_mut(), &mut context), std::task::Poll::Ready(42));
}
"#);
    compile_and_run(code);
}

#[test]
fn reassignment_of_the_same_async_function_through_aliases_compiles_and_runs() {
    let source = "async def value() -> i64:\n    return 42\nasync def answer() -> i64:\n    selected = value\n    alias = selected\n    selected = alias\n    if True:\n        selected = value\n    for number in range(2):\n        selected = alias\n    return await selected()\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let mut code = emit::rust(&low).unwrap();
    code.push_str(r#"
#[test] fn alias_calls() {
    let mut future = std::pin::pin!(answer());
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert_eq!(std::future::Future::poll(future.as_mut(), &mut context), std::task::Poll::Ready(42));
}
"#);
    compile_and_run(code);
}
