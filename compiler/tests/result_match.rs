use nagic::{check, emit, parser};

fn checked(source: &str) -> Result<nagic::ast::Program, String> {
    let mut p = parser::parse(source, true)?;
    check::check(&mut p)?;
    Ok(p)
}

#[test]
fn high_low_roundtrip_and_both_arms_return_values() {
    let high = checked("def choose(r: Result[i64, Error]) -> i64:\n    match r:\n        case Err(problem):\n            print(error_kind(problem))\n            return -1\n        case Ok(value):\n            value += 1\n            return value\n").unwrap();
    let low = emit::low(&high);
    assert!(low.contains("case Err(problem)"));
    let mut parsed = parser::parse(&low, false).unwrap();
    check::check(&mut parsed).unwrap();
    let rust = emit::rust(&parsed).unwrap();
    assert!(rust.contains("Err(mut problem) =>"));
    assert!(rust.contains("Ok(mut value) =>"));
}

#[test]
fn match_requires_result_and_exactly_one_of_each_arm() {
    for (source, message) in [
        ("def main():\n    match 1:\n        case Ok(value):\n            print(value)\n        case Err(_):\n            print(0)\n", "対象はResult"),
        ("def main():\n    match parse_i64(\"1\"):\n        case Ok(value):\n            print(value)\n", "両方"),
        ("def main():\n    match parse_i64(\"1\"):\n        case Ok(value):\n            print(value)\n        case Ok(other):\n            print(other)\n        case Err(_):\n            print(0)\n", "重複"),
        ("def main():\n    match parse_i64(\"1\"):\n        case Some(value):\n            print(value)\n", "OkまたはErr"),
        ("def main():\n    match parse_i64(\"1\"):\n        case _:\n            print(0)\n", "OkまたはErr"),
    ] { assert!(checked(source).unwrap_err().contains(message), "{source}"); }
}

#[test]
fn bindings_have_payload_types_and_do_not_escape_or_shadow() {
    for (source, message) in [
        ("def main():\n    match parse_i64(\"1\"):\n        case Ok(value):\n            x: str = value\n        case Err(_):\n            print(0)\n", "expected str"),
        ("def main():\n    match parse_i64(\"1\"):\n        case Ok(_):\n            print(1)\n        case Err(problem):\n            print(problem)\n", "printにErrorは渡せません"),
        ("def main():\n    match parse_i64(\"1\"):\n        case Ok(value):\n            print(value)\n        case Err(_):\n            print(0)\n    print(value)\n", "未定義"),
        ("def main():\n    value = 1\n    match parse_i64(\"1\"):\n        case Ok(value):\n            print(value)\n        case Err(_):\n            print(0)\n", "外側"),
    ] { assert!(checked(source).unwrap_err().contains(message), "{source}"); }
}

#[test]
fn source_and_owned_payloads_are_consumed_and_branch_moves_merge() {
    let cases = [
        "def main():\n    r = parse_i64(\"1\")\n    match r:\n        case Ok(value):\n            print(value)\n        case Err(_):\n            print(0)\n    r2 = r\n",
        "def take(value: str):\n    print(value)\ndef main():\n    r = ok(\"text\")\n    match r:\n        case Ok(value):\n            take(value)\n            print(value)\n        case Err(_):\n            print(0)\n",
        "def take(value: str):\n    print(value)\ndef main():\n    text = \"text\"\n    match parse_i64(\"1\"):\n        case Ok(_):\n            take(text)\n        case Err(_):\n            print(0)\n    print(text)\n",
    ];
    for source in cases {
        assert!(checked(source).unwrap_err().contains("move後"), "{source}");
    }
}

#[test]
fn views_in_result_payloads_keep_borrowing_until_the_arm_ends() {
    let good = "def main():\n    text = \"hello\"\n    r = slice(view(text), 0, 2)\n    match r:\n        case Ok(part):\n            print(copy(part))\n        case Err(_):\n            print(0)\n    text = \"changed\"\n    print(text)\n";
    checked(good).unwrap();
    let bad = good.replace(
        "print(copy(part))",
        "text = \"changed too soon\"\n            print(copy(part))",
    );
    assert!(checked(&bad).unwrap_err().contains("参照中"));
}

#[test]
fn nested_match_async_and_complete_return_paths_are_checked() {
    checked("async def value() -> Result[i64, Error]:\n    return ok(21)\nasync def choose() -> i64:\n    match await value():\n        case Ok(number):\n            match parse_i64(\"2\"):\n                case Ok(factor):\n                    return number * factor\n                case Err(_):\n                    return -1\n        case Err(_):\n            return -2\n").unwrap();
    let missing = "def choose() -> i64:\n    match parse_i64(\"1\"):\n        case Ok(value):\n            return value\n        case Err(_):\n            print(0)\n";
    assert!(checked(missing).unwrap_err().contains("すべての経路"));
}

#[test]
fn error_helpers_preserve_types_and_ownership() {
    checked("def recover(r: Result[str, Error]) -> Result[str, Error]:\n    match r:\n        case Ok(value):\n            return ok(value)\n        case Err(problem):\n            print(error_kind(problem))\n            print(error_message(problem))\n            return fail(problem)\ndef missing() -> Result[i64, Error]:\n    return not_found(\"missing\")\ndef failed() -> Result[i64, Error]:\n    return internal_error(\"failed\")\n").unwrap();
    assert!(checked("def main():\n    r = fail(1)\n")
        .unwrap_err()
        .contains("expected Error"));
    assert!(
        checked("def bad() -> Result[i64, i64]:\n    return not_found(\"missing\")\n")
            .unwrap_err()
            .contains("expected Error")
    );
}
