use nagic::{check, emit, parser};

#[test]
fn output_rejects_non_displayable_values_in_high_and_low() {
    for output in ["print", "write"] {
        for ty in [
            "unit",
            "view[i64]",
            "view[bytes]",
            "fn[i64]",
            "timestamp",
            "bytes",
            "List[i64]",
            "Error",
            "i64?",
            "Result[i64, Error]",
        ] {
            for (source, high) in [
                (
                    format!("def show(value: {ty}):\n    {output}(value)\n"),
                    true,
                ),
                (
                    format!("fn show(value: {ty}) -> unit {{\n    {output}(value);\n}}\n"),
                    false,
                ),
            ] {
                let mut program = parser::parse(&source, high).unwrap();
                let error = check::check(&mut program).expect_err(&source);
                assert!(error.starts_with("line 2:"), "{error}");
                assert!(
                    error.contains(&format!("{output}に{ty}は渡せません")),
                    "{error}"
                );
            }
        }
    }
}

#[test]
fn output_rejects_unit_calls_and_function_names() {
    for output in ["print", "write"] {
        for (source, ty) in [
            (
                format!("def empty():\n    return\ndef main():\n    {output}(empty())\n"),
                "unit",
            ),
            (
                format!("def value() -> i64:\n    return 1\ndef main():\n    {output}(value)\n"),
                "fn[i64]",
            ),
        ] {
            let mut program = parser::parse(&source, true).unwrap();
            let error = check::check(&mut program).expect_err(&source);
            assert!(error.starts_with("line 4:"), "{error}");
            assert!(
                error.contains(&format!("{output}に{ty}は渡せません")),
                "{error}"
            );
        }
    }
}

#[test]
fn output_keeps_all_supported_types_and_does_not_move_strings() {
    for ty in [
        "i8",
        "i16",
        "i32",
        "i64",
        "u8",
        "u16",
        "u32",
        "u64",
        "f32",
        "f64",
        "bool",
        "str",
        "view[str]",
        "UUID",
    ] {
        let source = format!("def show(value: {ty}):\n    write(value)\n    print(value)\n");
        let mut high = parser::parse(&source, true).unwrap();
        check::check(&mut high).unwrap();
        let mut low = parser::parse(&emit::low(&high), false).unwrap();
        check::check(&mut low).unwrap();
    }
}

#[test]
fn user_functions_named_print_and_write_keep_their_own_signatures() {
    let source = "def print(values: view[i64]) -> i64:\n    return len(values)\ndef write(values: view[i64]) -> i64:\n    return print(values)\ndef main() -> i64:\n    values = [1, 2]\n    return write(view(values))\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
}
