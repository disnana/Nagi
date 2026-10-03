use nagic::{ast::*, parser};

fn source(high: bool, body: &str) -> String {
    if high {
        format!("def main():\n{body}\n")
    } else {
        format!("fn main() {{\n{body}\n}}\n")
    }
}

#[test]
fn import_forms_keep_original_names_aliases_and_token_spans() {
    for high in [true, false] {
        let terminator = if high { "" } else { ";" };
        let program = parser::parse(
            &format!(
                "import \"legacy.nagi\"{terminator}\n\
                 import \"orders.nagi\" as orders{terminator}\n\
                 from \"orders.nagi\" import Order as SavedOrder{terminator}\n\
                 from \"orders.nagi\" import save{terminator}\n"
            ),
            high,
        )
        .unwrap();
        assert_eq!(program.imports, vec![("legacy.nagi".into(), 1)]);
        assert_eq!(program.module_imports.len(), 4);
        let flat = &program.module_imports[0];
        assert_eq!(flat.path, "legacy.nagi");
        assert_eq!(flat.line, 1);
        assert!(matches!(flat.kind, ImportKind::Flat));
        assert_eq!(flat.span.end - flat.span.start, 2);
        let module = &program.module_imports[1];
        let ImportKind::Module { alias, alias_span } = &module.kind else {
            panic!("expected module alias");
        };
        assert_eq!(alias, "orders");
        assert_eq!(alias_span.end - alias_span.start, 1);
        assert_eq!(alias_span.end, module.span.end);
        assert_eq!(module.span.end - module.span.start, 4);
        let named = &program.module_imports[2];
        let ImportKind::Names(names) = &named.kind else {
            panic!("expected named import");
        };
        assert_eq!(names.len(), 1);
        assert_eq!(names[0].name, "Order");
        assert_eq!(names[0].alias, "SavedOrder");
        assert_eq!(names[0].name_span.start, named.span.start + 3);
        assert_eq!(names[0].alias_span.start, named.span.start + 5);
        let ImportKind::Names(names) = &program.module_imports[3].kind else {
            panic!("expected unaliased named import");
        };
        assert_eq!(names[0].name, "save");
        assert_eq!(names[0].alias, "save");
        assert_eq!(names[0].name_span.start, names[0].alias_span.start);
        assert_eq!(names[0].name_span.end, names[0].alias_span.end);
    }
}

#[test]
fn from_and_as_remain_contextual_identifiers() {
    for high in [true, false] {
        let program = parser::parse(
            if high {
                "import \"orders.nagi\" as as\n\
                 from \"orders.nagi\" import as as from\n\
                 class from:\n    as: i64\n\
                 def as(from: from) -> i64:\n    as = from.as\n    return as\n"
            } else {
                "import \"orders.low\" as as;\n\
                 from \"orders.low\" import as as from;\n\
                 record from { as: i64; }\n\
                 fn as(from: from) -> i64 { let as = from.as; return as; }\n"
            },
            high,
        )
        .unwrap();
        assert_eq!(program.classes[0].name, "from");
        assert_eq!(program.classes[0].fields[0].0, "as");
        assert_eq!(program.functions[0].name, "as");
        assert_eq!(program.functions[0].params[0].0, "from");
        let S::Assign { name, value, .. } = &program.functions[0].body[0].kind else {
            panic!("expected assignment");
        };
        assert_eq!(name, "as");
        assert!(matches!(&value.kind, E::Field(receiver, member)
            if matches!(&receiver.kind, E::Name(name) if name == "from") && member == "as"));
    }
}

#[test]
fn qualified_types_parse_inside_nested_generics_lists_and_nullable_types() {
    let expected = Type::generic(
        "Result",
        vec![
            Type::generic(
                "List",
                vec![Type::generic("Option", vec![Type::named("orders.Order")])],
            ),
            Type::named("errors.Error"),
        ],
    );
    for high in [true, false] {
        let program = parser::parse(
            if high {
                "class State:\n    orders: Result[[orders.Order?], errors.Error]\n\
                 def save(value: Result[List[orders.Order?], errors.Error]) -> orders.Order?:\n    return None\n"
            } else {
                "record State { orders: Result[[orders.Order?], errors.Error]; }\n\
                 fn save(value: Result[List[orders.Order?], errors.Error]) -> orders.Order? { return null; }\n"
            },
            high,
        )
        .unwrap();
        assert_eq!(program.classes[0].fields[0].1, expected);
        assert_eq!(program.functions[0].params[0].1, expected);
        assert_eq!(
            program.functions[0].ret,
            Type::generic("Option", vec![Type::named("orders.Order")])
        );
    }
}

#[test]
fn qualified_calls_constructors_and_function_values_preserve_semantic_choice() {
    for high in [true, false] {
        let program = parser::parse(
            &source(
                high,
                concat!(
                    "    value = orders.save[orders.Order?](order);\n",
                    "    record = orders.Order(id=1);\n",
                    "    handler = orders.save;\n",
                    "    field = value.id;\n",
                    "    other = value.method();",
                ),
            ),
            high,
        )
        .unwrap();
        let values: Vec<&Expr> = program.functions[0]
            .body
            .iter()
            .map(|statement| match &statement.kind {
                S::Assign { value, .. } => value,
                _ => panic!("expected assignment"),
            })
            .collect();
        assert!(matches!(&values[0].kind, E::Call(name, types, args)
            if name == "orders.save"
                && types == &[Type::generic("Option", vec![Type::named("orders.Order")])]
                && args.len() == 1));
        assert!(matches!(&values[1].kind, E::Record(name, fields)
            if name == "orders.Order" && fields[0].0 == "id"));
        assert!(matches!(&values[2].kind, E::Field(receiver, member)
            if matches!(&receiver.kind, E::Name(name) if name == "orders") && member == "save"));
        assert!(matches!(&values[3].kind, E::Field(receiver, member)
            if matches!(&receiver.kind, E::Name(name) if name == "value") && member == "id"));
        // The resolver distinguishes a module alias from a lexical local and
        // rejects local method calls; the parser retains the entire target.
        assert!(matches!(&values[4].kind, E::Call(name, _, _) if name == "value.method"));
    }
}

#[test]
fn qualified_index_lookahead_restores_depth_and_preserves_index_nodes() {
    for high in [true, false] {
        let mut body = String::new();
        for _ in 0..90 {
            body.push_str("    value = orders.items[0];\n");
        }
        body.push_str("    value = orders.make[List[orders.Order?]]();");
        let program = parser::parse(&source(high, &body), high).unwrap();
        let S::Assign { value, .. } = &program.functions[0].body[0].kind else {
            panic!("expected assignment");
        };
        assert!(matches!(&value.kind, E::Index(receiver, _)
            if matches!(&receiver.kind, E::Field(_, member) if member == "items")));
        let S::Assign { value, .. } = &program.functions[0].body[90].kind else {
            panic!("expected final assignment");
        };
        assert!(
            matches!(&value.kind, E::Call(name, types, _) if name == "orders.make" && types.len() == 1)
        );
    }
}

#[test]
fn malformed_imports_and_qualified_constructs_keep_parser_errors() {
    for high in [true, false] {
        for statement in [
            "import orders.nagi as orders",
            "import \"orders.nagi\" as",
            "import \"orders.nagi\" as orders extra",
            "from orders.nagi import Order",
            "from \"orders.nagi\" Order",
            "from \"orders.nagi\" import",
            "from \"orders.nagi\" import Order as",
            "from \"orders.nagi\" import Order, save",
            "from \"orders.nagi\" import *",
        ] {
            assert!(
                parser::parse(&format!("{statement}\n"), high).is_err(),
                "{statement}"
            );
        }
        for body in [
            "    value = orders.Order[i64](id=1);",
            "    value = orders.save(order, id=1);",
            "    value = orders.(1);",
            "    value = [orders][0].save();",
        ] {
            assert!(parser::parse(&source(high, body), high).is_err(), "{body}");
        }
        let source = if high {
            "@replace generated::orders::save\nimport \"orders.nagi\" as orders\n"
        } else {
            "@replace generated::orders::save;\nimport \"orders.low\" as orders;\n"
        };
        assert!(parser::parse(source, high)
            .unwrap_err()
            .contains("importに属性は付けられません"));
    }
}

#[test]
fn qualified_types_respect_existing_nesting_limit() {
    for high in [true, false] {
        let ty = format!("{}orders.Order{}", "List[".repeat(65), "]".repeat(65));
        let input = if high {
            format!("def deep(value: {ty}):\n    print(0)\n")
        } else {
            format!("fn deep(value: {ty}) {{ print(0); }}\n")
        };
        assert!(parser::parse(&input, high)
            .unwrap_err()
            .contains("型の入れ子は64段までです"));
    }
}

#[test]
fn low_metadata_is_read_without_changing_source_spans() {
    let metadata = ModuleMetadata {
        root: Some(ModuleId("original/main.nagi".into())),
        modules: vec![ModuleInfo {
            id: ModuleId("original/main.nagi".into()),
            path: "original/main.nagi".into(),
        }],
        ..ModuleMetadata::default()
    };
    let input = format!(
        "# nagi-modules-v1 {}\nimport \"legacy.low\";\nfn main() {{ print(0); }}\n",
        serde_json::to_string(&metadata).unwrap(),
    );
    let program = parser::parse(&input, false).unwrap();
    assert_eq!(program.modules.root, metadata.root);
    assert_eq!(program.modules.modules[0].id, metadata.modules[0].id);
    assert_eq!(program.functions[0].line, 3);
    assert_eq!(program.module_imports[0].line, 2);
    assert_eq!(program.module_imports[0].span.start, 0);
    let duplicate = format!(
        "# nagi-modules-v1 {}\n# nagi-modules-v1 {}\n",
        serde_json::to_string(&metadata).unwrap(),
        serde_json::to_string(&metadata).unwrap(),
    );
    assert!(parser::parse(&duplicate, false)
        .unwrap_err()
        .contains("header is duplicated"));
    for input in [
        "# nagi-modules-v1 {}\n",
        "# nagi-modules-v1 invalid\n",
        "# nagi-modules-v1\n",
    ] {
        assert!(parser::parse(input, false).is_err(), "{input}");
        assert!(
            parser::parse(input, true).is_ok(),
            "High comments stay comments: {input}"
        );
    }
}

#[test]
fn module_replace_path_remains_a_structured_attribute_value() {
    for high in [true, false] {
        let input = if high {
            "@replace generated::orders::save\ndef save() -> i64:\n    return 2\n"
        } else {
            "@replace generated::orders::save;\nfn save() -> i64 { return 2; }\n"
        };
        let program = parser::parse(input, high).unwrap();
        assert_eq!(
            program.functions[0].attrs,
            vec![("replace".into(), "generated::orders::save".into())]
        );
    }
}
