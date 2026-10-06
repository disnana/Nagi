mod resource_contract_goldens;
mod checked_emission {
    pub fn seal(program: &crate::ast::Program) -> crate::checked::CheckedProgram {
        crate::check::finalize(
            program.clone(),
            crate::ast::Program::default(),
            crate::source::SourceProvenance::user_low_unmapped(),
        )
        .unwrap_or_else(|error| panic!("fixture finalization failed: {error}"))
    }
}
use crate::{check, emit, parser};
fn high(s: &str) -> Result<crate::ast::Program, String> {
    let mut p = parser::parse(s, true)?;
    check::check(&mut p)?;
    Ok(p)
}
#[test]
fn hello() {
    high("def main():\n    print(\"hello\")\n").unwrap();
}
#[test]
fn rust_extern_interface_and_low_roundtrip() {
    let p = high("@rust(\"native::copy_text\")\nextern def copy_text(s: view[str]) -> str\ndef main():\n    print(copy_text(view(\"hello\")))\n").unwrap();
    let mut low = parser::parse(&emit::low(&p), false).unwrap();
    check::check(&mut low).unwrap();
    assert!(low.functions[0].external);
    assert!(emit::rust(&checked_emission::seal(&low))
        .unwrap()
        .contains("native::copy_text(s)"));
    assert!(high("extern def missing() -> i64\n").is_err());
    assert!(high("@rust(\"native::f(); panic!()\")\nextern def bad() -> i64\n").is_err());
    assert!(
        high("@rust(\"native::f\")\nextern def borrowed(x: view[str]) -> view[str]\n").is_err()
    );
    assert!(high("@rust(\"native::f\")\ndef normal() -> i64:\n    return 1\n").is_err());
}

#[test]
fn html_route_is_not_encoded_as_json() {
    let p = high("@get(\"/\")\nasync def home() -> Result[Html, Error]:\n    return ok(html(\"<h1>Hello</h1>\"))\n").unwrap();
    let code = emit::rust(&checked_emission::seal(&p)).unwrap();
    assert!(code.contains("IntoResponse::into_response(v)"));
    assert!(high("def main():\n    html(123)\n").is_err());
    assert!(
        high("def main():\n    path = \"index.html\"\n    print(include_text(path))\n").is_err()
    );
}
#[test]
fn console_io_roundtrip() {
    let p = high("def main() -> Result[unit, Error]:\n    write(\"prompt: \")\n    text = try read_line()\n    return ok(print(text))\n").unwrap();
    let mut low = parser::parse(&emit::low(&p), false).unwrap();
    check::check(&mut low).unwrap();
    let rust = emit::rust(&checked_emission::seal(&low)).unwrap();
    assert!(rust.contains("::nagi_runtime::read_line()"));
    assert!(high("def main():\n    read_line()\n").is_err());
    assert!(high("def main():\n    x = read_line(1)\n").is_err());
    assert!(high("def main():\n    write([1, 2])\n").is_err());
}
#[test]
fn precedence() {
    let p = high("def main() -> i64:\n    return 2 + 3 * 4\n").unwrap();
    assert!(emit::low(&p).contains("(2 + (3 * 4))"));
}
#[test]
fn roundtrip_low() {
    let p=high("class Point:\n    x: f64\n    y: f64\ndef add(x: i64, y: i64) -> i64:\n    if x > y:\n        return x\n    else:\n        return y\n").unwrap();
    let mut q = parser::parse(&emit::low(&p), false).unwrap();
    check::check(&mut q).unwrap();
    assert_eq!(q.classes[0].fields.len(), 2);
}
#[test]
fn invalid_indent() {
    assert!(high("def main():\n    x = 1\n   print(x)\n").is_err());
}
#[test]
fn tabs_rejected() {
    assert!(high("def main():\n\tprint(1)\n").is_err());
}
#[test]
fn bad_string() {
    assert!(high("def main():\n    print(\"bad)\n").is_err());
}
#[test]
fn unknown_name() {
    assert!(high("def main():\n    print(x)\n").is_err());
}
#[test]
fn mismatched_type() {
    assert!(high("def main():\n    x: i32 = \"bad\"\n").is_err());
}
#[test]
fn unknown_class() {
    assert!(high("def f(x: Mystery):\n    print(1)\n").is_err());
}
#[test]
fn native_numeric() {
    high("def f(x: i32) -> i64:\n    return i64(x)\n").unwrap();
}
#[test]
fn no_implicit_conversion() {
    assert!(high("def f(x: i32) -> i64:\n    return x\n").is_err());
}
#[test]
fn number_overflow() {
    assert!(high("def f() -> i8:\n    return 999\n").is_err());
}
#[test]
fn floating_literals_reject_f32_overflow_in_high_and_low() {
    let overflow = "400000000000000000000000000000000000000.0";
    for literal in [overflow.to_owned(), format!("-{overflow}")] {
        for (source, is_high) in [
            (format!("def f() -> f32:\n    return {literal}\n"), true),
            (format!("fn f() -> f32 {{ return {literal}; }}\n"), false),
            (format!("def main():\n    value: f32 = {literal}\n"), true),
            (format!("def take(value: f32):\n    print(value)\ndef main():\n    take({literal})\n"), true),
            (format!("class Number:\n    value: f32\ndef main():\n    value = Number(value={literal})\n"), true),
            (format!("def main():\n    values: List[f32] = [{literal}]\n"), true),
        ] {
            let mut program = parser::parse(&source, is_high).unwrap();
            let error = check::check(&mut program).expect_err(&source);
            assert!(error.contains("浮動小数リテラルが範囲外"), "{error}");
            assert!(error.starts_with("line "), "{error}");
        }
    }
}
#[test]
fn floating_literals_accept_f32_boundary_and_roundtrip() {
    let maximum = "340282346638528859811704183484516925440.0";
    for literal in [
        "0.0".to_owned(),
        "1.5".to_owned(),
        maximum.to_owned(),
        format!("-{maximum}"),
    ] {
        let program = high(&format!("def f() -> f32:\n    return {literal}\n")).unwrap();
        let mut low = parser::parse(&emit::low(&program), false).unwrap();
        check::check(&mut low).unwrap();
        assert_eq!(low.functions[0].ret, crate::ast::Type::named("f32"));
    }
}
#[test]
fn floating_literals_keep_f64_range_and_default_inference() {
    let outside_f32 = "400000000000000000000000000000000000000.0";
    let inferred = high(&format!("def main():\n    value = {outside_f32}\n")).unwrap();
    assert!(emit::low(&inferred).contains("let value: f64"));
    high(&format!("def f() -> f64:\n    return {outside_f32}\n")).unwrap();
    let finite = format!("1{}.0", "0".repeat(308));
    high(&format!("def f() -> f64:\n    return {finite}\n")).unwrap();
    let overflow = format!("1{}.0", "0".repeat(309));
    assert!(high(&format!("def f() -> f64:\n    return {overflow}\n")).is_err());
}
#[test]
fn class_field_check() {
    assert!(high("class A:\n    x: i64\ndef f() -> A:\n    return A(y=1)\n").is_err());
}
#[test]
fn duplicate_class_fields() {
    assert!(high("class A:\n    x: i64\n    x: i32\n").is_err());
}
#[test]
fn recursive_value() {
    assert!(high("class A:\n    other: A\n").is_err());
}
#[test]
fn class_move() {
    assert!(high("class A:\n    x: str\ndef take(x: A):\n    print(x.x)\ndef main():\n    a = A(x=\"owned\")\n    take(a)\n    take(a)\n").is_err());
}
#[test]
fn string_move() {
    assert!(high("def take(s: str):\n    print(s)\ndef main():\n    s = \"hello\"\n    take(s)\n    print(s)\n").is_err());
}
#[test]
fn view_escape() {
    assert!(
        high("def f() -> view[str]:\n    s = \"local\"\n    return view(s)\n")
            .unwrap_err()
            .contains("escapes")
    );
}
#[test]
fn view_return_parameter() {
    high("def f(s: view[str]) -> view[str]:\n    return s\n").unwrap();
}
#[test]
fn owned_parameter_cannot_return_view() {
    assert!(high("def f(s: str) -> view[str]:\n    return view(s)\n").is_err());
}
#[test]
fn nested_view_in_class_rejected() {
    assert!(high("class A:\n    text: view[str]?\n").is_err());
}
#[test]
fn copy_value_class() {
    high("class A:\n    x: i64\ndef use(a: A):\n    print(a.x)\ndef main():\n    a = A(x=1)\n    use(a)\n    use(a)\n").unwrap();
}
#[test]
fn view_prevents_move() {
    assert!(high("def take(s: str):\n    print(s)\ndef main():\n    s = \"hello\"\n    v = view(s)\n    take(s)\n").is_err());
}
#[test]
fn view_prevents_mutation() {
    assert!(high("def main():\n    s = [1, 2]\n    v = view(s)\n    append(s, 3)\n").is_err());
}
#[test]
fn explicit_copy() {
    high("def f() -> str:\n    s = \"local\"\n    return copy(view(s))\n").unwrap();
}
#[test]
fn await_only_async() {
    assert!(high("def main():\n    await sleep(1)\n").is_err());
}
#[test]
fn forgotten_await() {
    assert!(high("async def main():\n    sleep(1)\n").is_err());
}
#[test]
fn result_must_handle() {
    assert!(high("def main():\n    parse_i64(\"1\")\n").is_err());
}
#[test]
fn error_propagation() {
    high("def f() -> Result[i64, Error]:\n    n = try parse_i64(\"42\")\n    return ok(n)\n")
        .unwrap();
}
#[test]
fn scope_spawn() {
    high("async def main() -> Result[unit, Error]:\n    async with scope:\n        spawn sleep(1)\n        spawn sleep(2)\n    return ok(print(\"done\"))\n").unwrap();
}
#[test]
fn unscoped_spawn() {
    assert!(high("async def main():\n    spawn sleep(1)\n").is_err());
}
#[test]
fn view_cannot_spawn() {
    assert!(high("async def f(s: view[str]):\n    print(s)\nasync def main() -> Result[unit,Error]:\n    s = \"hello\"\n    v = view(s)\n    async with scope:\n        spawn f(v)\n    return ok(print(1))\n").is_err());
}
#[test]
fn missing_return() {
    assert!(high("def f(x: i64) -> i64:\n    if x > 1:\n        return x\n").is_err());
}
#[test]
fn nullable() {
    high("def f() -> i64?:\n    return None\n").unwrap();
}
#[test]
fn empty_list_inference() {
    assert!(high("def main():\n    xs = []\n").is_err());
    high("def main():\n    xs: List[i64] = []\n    append(xs, 1)\n").unwrap();
}
#[test]
fn replace_exact_signature() {
    let mut p = high("def f(x: i64) -> i64:\n    return x + 1\n").unwrap();
    let q = parser::parse(
        "@replace generated::f\nfn f(x: i64) -> i64 { return x * 2; }",
        false,
    )
    .unwrap();
    check::integrate(&mut p, q).unwrap();
    assert!(emit::low(&p).contains("x * 2"));
}
#[test]
fn replace_bad_signature() {
    let mut p = high("def f(x: i64) -> i64:\n    return x\n").unwrap();
    let q = parser::parse(
        "@replace generated::f\nfn f(x: i32) -> i64 { return i64(x); }",
        false,
    )
    .unwrap();
    assert!(check::integrate(&mut p, q).is_err());
}
#[test]
fn low_standalone() {
    let mut p = parser::parse(
        "record P { x: i64; y: i64; } fn main() -> unit { let x: i64 = 5; print(x); }",
        false,
    )
    .unwrap();
    check::check(&mut p).unwrap();
}
#[test]
fn cost_not_dynamic_count() {
    let p = high("def main():\n    s = \"hello\"\n    print(s)\n").unwrap();
    assert_eq!(emit::cost_report(&p)["format"], "nagi-cost-sites-v1");
}

#[test]
fn signed_minimum_literals_are_in_range_in_high_and_low() {
    for (ty, minimum, below) in [
        ("i8", "128", "129"),
        ("i16", "32768", "32769"),
        ("i32", "2147483648", "2147483649"),
        ("i64", "9223372036854775808", "9223372036854775809"),
    ] {
        let program = high(&format!("def minimum() -> {ty}:\n    return -{minimum}\n")).unwrap();
        let mut low = parser::parse(&emit::low(&program), false).unwrap();
        check::check(&mut low).unwrap();
        assert!(high(&format!("def too_low() -> {ty}:\n    return -{below}\n")).is_err());
        assert!(high(&format!("def too_high() -> {ty}:\n    return {minimum}\n")).is_err());
    }
    high("def main():\n    minimum = -9223372036854775808\n").unwrap();
    assert!(high("def unsigned() -> u8:\n    return -1\n").is_err());
}

#[test]
fn signed_minimum_literals_compile_and_run_after_lowering() {
    let program = high("def min_i8() -> i8:\n    return -128\ndef min_i16() -> i16:\n    return -32768\ndef min_i32() -> i32:\n    return -2147483648\ndef min_i64() -> i64:\n    return -9223372036854775808\ndef main():\n    print(min_i8())\n    print(min_i16())\n    print(min_i32())\n    print(min_i64())\n").unwrap();
    let mut low = parser::parse(&emit::low(&program), false).unwrap();
    check::check(&mut low).unwrap();
    let folder = std::env::temp_dir().join(format!(
        "nagi signed min {} {}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&folder).unwrap();
    let source = folder.join("main.rs");
    let binary = folder.join(format!("minimum{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&source, emit::rust(&checked_emission::seal(&low)).unwrap()).unwrap();
    let build = std::process::Command::new("rustc")
        .arg("--edition=2021")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = std::process::Command::new(&binary).output().unwrap();
    assert!(output.status.success());
    let actual: Vec<i64> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| line.parse().unwrap())
        .collect();
    assert_eq!(
        actual,
        vec![i8::MIN as i64, i16::MIN as i64, i32::MIN as i64, i64::MIN]
    );
    std::fs::remove_dir_all(folder).unwrap();
}
