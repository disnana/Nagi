use nagic::{check, emit, parser};

#[test]
fn negation_rejects_non_numeric_types_regardless_of_their_names() {
    for ty in [
        "item",
        "flower",
        "fn[i64]",
        "bool",
        "u64",
        "str",
        "UUID",
        "timestamp",
    ] {
        for (prefix, high) in [
            (
                "class item:\n    value: i64\nclass flower:\n    value: i64\n",
                true,
            ),
            (
                "record item { value: i64; }\nrecord flower { value: i64; }\n",
                false,
            ),
        ] {
            let source = if high {
                format!("{prefix}def negative(value: {ty}) -> {ty}:\n    return -value\n")
            } else {
                format!("{prefix}fn negative(value: {ty}) -> {ty} {{\n    return -value;\n}}\n")
            };
            let mut program = parser::parse(&source, high).unwrap();
            let error = check::check(&mut program).expect_err(&source);
            assert!(
                error.contains(&format!("{ty}は符号反転できません")),
                "{error}"
            );
            assert!(
                error.starts_with(if high { "line 6:" } else { "line 4:" }),
                "{error}"
            );
        }
    }
    let mut program = parser::parse(
        "def value() -> i64:\n    return 1\ndef main():\n    result = -value\n",
        true,
    )
    .unwrap();
    assert!(check::check(&mut program)
        .unwrap_err()
        .contains("fn[i64]は符号反転できません"));
}

#[test]
fn comparison_checks_the_operation_and_borrowed_element_types() {
    for (ty, operators) in [
        ("UUID", vec!["<", ">", "<=", ">="]),
        ("timestamp", vec!["<", ">", "<=", ">="]),
        ("view[UUID]", vec!["<", ">", "<=", ">="]),
        ("view[timestamp]", vec!["<", ">", "<=", ">="]),
        ("view[Item]", vec!["==", "!=", "<", ">", "<=", ">="]),
        ("view[List[Item]]", vec!["==", "<"]),
        ("view[Option[Item]]", vec!["==", "<"]),
        ("view[Result[i64, Error]]", vec!["==", "<"]),
        ("view[Map[f64, i64]]", vec!["==", "!="]),
        ("view[Map[i64, i64]]", vec!["<", ">", "<=", ">="]),
    ] {
        for op in operators {
            for (prefix, high) in [
                ("class Item:\n    value: i64\n", true),
                ("record Item { value: i64; }\n", false),
            ] {
                let source = if high {
                    format!("{prefix}def compare(values: {ty}) -> bool:\n    return values {op} values\n")
                } else {
                    format!("{prefix}fn compare(values: {ty}) -> bool {{\n    return values {op} values;\n}}\n")
                };
                let mut program = parser::parse(&source, high).unwrap();
                let rendered = program.functions[0].params[0].1.to_string();
                let error = check::check(&mut program).expect_err(&source);
                assert!(
                    error.contains(&format!("{rendered}は{op}による比較に対応していません")),
                    "{error}"
                );
            }
        }
    }
}

#[test]
fn uuid_and_timestamp_equality_and_supported_views_still_check() {
    for ty in [
        "UUID",
        "timestamp",
        "view[UUID]",
        "view[timestamp]",
        "view[i64]",
        "view[bytes]",
        "view[str]",
        "view[Option[f64]]",
        "view[Result[i64, i64]]",
        "view[Map[i64, str]]",
        "view[shared[i64]]",
        "view[owned[i64]]",
    ] {
        let source = format!("def compare(value: {ty}) -> bool:\n    return value == value and not (value != value)\n");
        let mut high = parser::parse(&source, true).unwrap();
        check::check(&mut high).unwrap();
        let mut low = parser::parse(&emit::low(&high), false).unwrap();
        check::check(&mut low).unwrap();
    }
}
