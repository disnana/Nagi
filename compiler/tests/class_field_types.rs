use nagic::{check, emit, parser};

#[test]
fn unsupported_resource_and_function_fields_fail_at_the_field_line() {
    for ty in [
        "fn[i64, i64]",
        "List[fn[i64]]",
        "Option[fn[i64]]",
        "Db",
        "Html",
        "shared[Html]",
    ] {
        for (source, high) in [
            (
                format!("class Payload:\n    value: i64\n    unsupported: {ty}\n"),
                true,
            ),
            (
                format!("record Payload {{\n    value: i64;\n    unsupported: {ty};\n}}\n"),
                false,
            ),
        ] {
            let mut program = parser::parse(&source, high).unwrap();
            let error = check::check(&mut program).unwrap_err();
            assert!(
                error.starts_with("line 3:") && error.contains("classのフィールドに保存できません"),
                "{error}"
            );
        }
    }
}

#[test]
fn map_keys_with_missing_equality_or_hashing_fail_before_derivation() {
    for ty in [
        "f32",
        "f64",
        "UUID",
        "timestamp",
        "List[f64]",
        "Option[f32]",
        "shared[f64]",
        "Result[i64, f64]",
        "Map[i64, i64]",
    ] {
        let source = format!("class Lookup:\n    entries: Map[{ty}, i64]\n");
        let mut program = parser::parse(&source, true).unwrap();
        let error = check::check(&mut program).unwrap_err();
        assert!(
            error.starts_with("line 2:") && error.contains("classのMapフィールドのキー"),
            "{error}"
        );
    }
}

#[test]
fn nested_data_fields_and_bridge_defined_key_traits_remain_available() {
    let source = "class Key:\n    value: i64\nclass Payload:\n    values: shared[List[Result[i64, str]]]\n    names: Map[str, i64]\n    custom: Map[Key, str]\n    children: List[Payload]\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
}
