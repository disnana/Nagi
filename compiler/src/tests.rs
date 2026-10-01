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
    assert!(emit::rust(&low).unwrap().contains("native::copy_text(s)"));
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
    let code = emit::rust(&p).unwrap();
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
    let rust = emit::rust(&low).unwrap();
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
