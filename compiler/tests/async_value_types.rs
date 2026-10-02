use nagic::{check, emit, parser};

#[test]
fn async_function_signatures_and_container_types_fail_before_emission() {
    for (source, high) in [
        ("async def apply(callback: fn[i64, Future[i64]], value: i64) -> i64:\n    return await callback(value)\n", true),
        ("async fn apply(callback: fn[i64, Future[i64]], value: i64) -> i64 { return await callback(value); }\n", false),
        ("def choose() -> fn[i64, Future[i64]]:\n    return answer\nasync def answer(value: i64) -> i64:\n    return value\n", true),
        ("fn choose() -> fn[i64, Future[i64]] { return answer; }\nasync fn answer(value: i64) -> i64 { return value; }\n", false),
        ("class Callbacks:\n    callback: fn[i64, Future[i64]]\n", true),
        ("record Callbacks { callback: fn[i64, Future[i64]]; }\n", false),
        ("def apply(callbacks: List[fn[i64, Future[i64]]]):\n    print(1)\n", true),
        ("async def answer() -> i64:\n    return 1\ndef main():\n    callbacks = [answer]\n", true),
        ("async fn answer() -> i64 { return 1; }\nfn main() -> unit { let callbacks: List[fn[Future[i64]]] = [answer]; }\n", false),
    ] {
        let mut program = parser::parse(source, high).unwrap();
        let error = check::check(&mut program).expect_err(source);
        assert!(error.contains("async関数を引数・戻り値・コンテナー"), "{error}");
    }
}

#[test]
fn assigning_an_unawaited_future_reports_the_call_site() {
    for (source, high) in [
        (
            "async def main():\n    pending = sleep(1)\n    await pending\n",
            true,
        ),
        (
            "async fn main() -> unit {\n    let pending = sleep(1);\n    await pending;\n}\n",
            false,
        ),
    ] {
        let mut program = parser::parse(source, high).unwrap();
        let error = check::check(&mut program).expect_err(source);
        assert!(error.starts_with("line 2:"), "{error}");
        assert!(error.contains("呼び出し時にawaitしてください"), "{error}");
    }
}

#[test]
fn local_async_aliases_keep_inferred_and_explicit_types() {
    let source = "async def answer(value: i64) -> i64:\n    return value + 1\nasync def main():\n    inferred = answer\n    annotated: fn[i64, Future[i64]] = answer\n    print(await inferred(41))\n    print(await annotated(41))\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
}

#[test]
fn a_class_named_future_is_not_an_async_result() {
    let source = "class Future:\n    value: i64\ndef main():\n    result = Future(value=42)\n    print(result.value)\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
}
