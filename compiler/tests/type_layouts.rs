use nagic::{check, emit, parser};

#[test]
fn inline_wrappers_do_not_break_recursive_layouts_in_high_or_low() {
    for ty in [
        "Node?",
        "Option[Node]",
        "owned[Node]",
        "Result[i64, Node]",
        "Result[Node, Error]",
        "Option[owned[Node]]",
    ] {
        for (source, high) in [
            (format!("class Node:\n    child: {ty}\n"), true),
            (format!("record Node {{\n    child: {ty};\n}}\n"), false),
        ] {
            let mut program = parser::parse(&source, high).unwrap();
            let error = check::check(&mut program).expect_err(&source);
            assert!(
                error.starts_with("line 1:") && error.contains("再帰する値型レイアウト"),
                "{error}"
            );
        }
    }
}

#[test]
fn mixed_direct_and_nullable_class_cycles_are_rejected() {
    let source = "class First:\n    child: Second\nclass Second:\n    child: Third?\nclass Third:\n    child: First\n";
    let mut program = parser::parse(source, true).unwrap();
    assert!(check::check(&mut program)
        .unwrap_err()
        .contains("再帰する値型レイアウト"));
}

#[test]
fn indirect_storage_and_reused_non_recursive_types_remain_valid() {
    for ty in [
        "List[Node]",
        "Option[List[Node]]",
        "shared[Node]",
        "Option[shared[Node]]",
        "Map[str, Node]",
    ] {
        let source = format!("class Node:\n    children: {ty}\n");
        let mut high = parser::parse(&source, true).unwrap();
        check::check(&mut high).unwrap();
        let mut low = parser::parse(&emit::low(&high), false).unwrap();
        check::check(&mut low).unwrap();
    }
    let mut source = String::from("class Layer0:\n    value: i64\n");
    // A shared acyclic dependency graph should be checked once per class,
    // rather than revisiting the same two subgraphs at every level.
    for n in 1..24 {
        source.push_str(&format!(
            "class Layer{n}:\n    first: Layer{}\n    second: Layer{}\n",
            n - 1,
            n - 1
        ));
    }
    let mut program = parser::parse(&source, true).unwrap();
    check::check(&mut program).unwrap();
}

#[test]
fn invalid_field_types_point_at_the_field_instead_of_the_class_header() {
    for (source, high) in [
        (
            "class Example:\n    first: i64\n    second: Missing\n",
            true,
        ),
        (
            "record Example {\n    first: i64;\n    second: Missing;\n}\n",
            false,
        ),
    ] {
        let mut program = parser::parse(source, high).unwrap();
        let error = check::check(&mut program).unwrap_err();
        assert!(
            error.starts_with("line 3:") && error.contains("未定義の型: Missing"),
            "{error}"
        );
    }
}
