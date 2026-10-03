use nagic::{ast::*, lexer, parser};

fn wrap_body(high: bool, body: &str) -> String {
    if high {
        format!("def main():\n{body}\n")
    } else {
        format!("fn main() {{\n{body}\n}}\n")
    }
}

#[test]
fn enums_keep_variant_names_fields_types_and_source_locations() {
    for high in [true, false] {
        let source = if high {
            "enum AuthError:\n    InvalidCredentials\n    WeakPassword(message: str)\n    Retry(after: i64, cause: Result[unit, errors.Cause]?)\n"
        } else {
            "enum AuthError {\n    InvalidCredentials;\n    WeakPassword(message: str);\n    Retry(after: i64, cause: Result[unit, errors.Cause]?);\n}\n"
        };
        let tokens = lexer::lex(source, high).unwrap();
        let program = parser::parse(source, high).unwrap();
        let error = &program.enums[0];
        assert_eq!(error.name, "AuthError");
        assert_eq!(error.line, 1);
        assert_eq!(error.variants.len(), 3);
        for (index, variant) in error.variants.iter().enumerate() {
            assert_eq!(variant.line, index + 2);
            assert_eq!(variant.name_span.end - variant.name_span.start, 1);
            assert_eq!(
                tokens[variant.name_span.start].kind,
                lexer::K::Id(variant.name.clone())
            );
        }
        assert!(error.variants[0].fields.is_empty());
        assert!(error.variants[0].field_lines.is_empty());
        assert_eq!(
            error.variants[1].fields,
            vec![("message".into(), Type::named("str"))]
        );
        assert_eq!(error.variants[1].field_lines, vec![3]);
        assert_eq!(error.variants[2].field_lines, vec![4, 4]);
        assert_eq!(
            error.variants[2].fields[1],
            (
                "cause".into(),
                Type::generic(
                    "Option",
                    vec![Type::generic(
                        "Result",
                        vec![Type::named("unit"), Type::named("errors.Cause")]
                    )]
                )
            )
        );
    }
}

#[test]
fn enum_constructors_keep_the_existing_expression_forms() {
    for high in [true, false] {
        let program = parser::parse(
            &wrap_body(
                high,
                "    unit = errors.AuthError.InvalidCredentials;\n    positional = errors.AuthError.WeakPassword(\"short\");\n    named = errors.AuthError.WeakPassword(message=\"short\");",
            ),
            high,
        )
        .unwrap();
        let expressions: Vec<&Expr> = program.functions[0]
            .body
            .iter()
            .map(|statement| match &statement.kind {
                S::Assign { value, .. } => value,
                _ => panic!("expected assignment"),
            })
            .collect();
        assert!(matches!(&expressions[0].kind, E::Field(base, variant)
            if variant == "InvalidCredentials"
                && matches!(&base.kind, E::Field(module, name)
                    if name == "AuthError"
                        && matches!(&module.kind, E::Name(name) if name == "errors"))));
        assert!(matches!(&expressions[1].kind, E::Call(name, types, values)
            if name == "errors.AuthError.WeakPassword" && types.is_empty() && values.len() == 1));
        assert!(matches!(&expressions[2].kind, E::Record(name, fields)
            if name == "errors.AuthError.WeakPassword" && fields[0].0 == "message"));
    }
}

#[test]
fn enum_patterns_keep_qualified_names_and_each_payload_binding_span() {
    for high in [true, false] {
        let source = if high {
            "def describe(value: errors.AuthError) -> i64:\n    match value:\n        case errors.AuthError.InvalidCredentials:\n            return 0\n        case errors.AuthError.Retry(after, _):\n            return after\n"
        } else {
            "fn describe(value: errors.AuthError) -> i64 {\n    match value {\n        case errors.AuthError.InvalidCredentials { return 0; }\n        case errors.AuthError.Retry(after, _) { return after; }\n    }\n}\n"
        };
        let tokens = lexer::lex(source, high).unwrap();
        let program = parser::parse(source, high).unwrap();
        let S::Match(_, arms) = &program.functions[0].body[0].kind else {
            panic!("expected match");
        };
        let MatchPattern::Enum {
            name,
            bindings,
            span,
        } = &arms[0].pattern
        else {
            panic!("expected unit enum pattern");
        };
        assert_eq!(name, "errors.AuthError.InvalidCredentials");
        assert!(bindings.is_empty());
        assert_eq!(span.end - span.start, 5);
        assert_eq!(arms[0].line, 3);
        let MatchPattern::Enum {
            name,
            bindings,
            span,
        } = &arms[1].pattern
        else {
            panic!("expected payload enum pattern");
        };
        assert_eq!(name, "errors.AuthError.Retry");
        assert_eq!(span.end - span.start, 5);
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].name.as_deref(), Some("after"));
        assert_eq!(bindings[1].name, None);
        for (binding, spelling) in bindings.iter().zip(["after", "_"]) {
            assert_eq!(binding.span.end - binding.span.start, 1);
            assert_eq!(
                tokens[binding.span.start].kind,
                lexer::K::Id(spelling.into())
            );
            assert_eq!(binding.ty, None);
        }
        assert_eq!(arms[1].pattern.bindings().len(), 2);
    }
}

#[test]
fn result_patterns_preserve_ok_err_and_discard_bindings() {
    for high in [true, false] {
        let source = if high {
            "def describe(value: Result[i64, Error]) -> i64:\n    match value:\n        case Ok(number):\n            return number\n        case Err(_):\n            return 0\n"
        } else {
            "fn describe(value: Result[i64, Error]) -> i64 {\n    match value {\n        case Ok(number) { return number; }\n        case Err(_) { return 0; }\n    }\n}\n"
        };
        let program = parser::parse(source, high).unwrap();
        let S::Match(_, arms) = &program.functions[0].body[0].kind else {
            panic!("expected match");
        };
        assert!(matches!(&arms[0].pattern,
            MatchPattern::Result { ok: true, binding }
                if binding.name.as_deref() == Some("number")));
        assert!(matches!(&arms[1].pattern,
            MatchPattern::Result { ok: false, binding }
                if binding.name.is_none()));
        assert_eq!(arms[0].pattern.bindings().len(), 1);
        assert_eq!(arms[1].pattern.bindings().len(), 1);
    }
}

#[test]
fn invalid_enum_declarations_return_parse_errors() {
    for high in [true, false] {
        for declaration in ["enum", "enum Error[T]", "enum Error: Variant"] {
            assert!(parser::parse(declaration, high).is_err(), "{declaration}");
        }
        for variant in [
            "Empty()",
            "Payload(value)",
            "Payload(value:)",
            "Payload(value: str,)",
            "Payload(value: str) extra",
            "Payload(value: str",
        ] {
            let source = if high {
                format!("enum Error:\n    {variant}\n")
            } else {
                format!("enum Error {{ {variant}; }}\n")
            };
            assert!(parser::parse(&source, high).is_err(), "{source}");
        }
        let attributed = if high {
            "@tag(\"error\")\nenum Error:\n    Failed\n"
        } else {
            "@tag(\"error\");\nenum Error { Failed; }\n"
        };
        assert!(parser::parse(attributed, high).is_err());
    }
}

#[test]
fn patterns_reject_wildcards_nested_matches_and_unnamed_variants() {
    for high in [true, false] {
        for pattern in [
            "_",
            "Failed",
            "AuthError.",
            "AuthError.Failed()",
            "AuthError.Payload(inner(value))",
            "AuthError.Payload(value=message)",
            "AuthError.Payload(\"literal\")",
            "AuthError.Payload(value,)",
            "AuthError.Payload(value) | AuthError.Failed",
            "Ok()",
            "Err(first, second)",
        ] {
            let source = if high {
                format!("def f(value: AuthError):\n    match value:\n        case {pattern}:\n            return\n")
            } else {
                format!(
                    "fn f(value: AuthError) {{ match value {{ case {pattern} {{ return; }} }} }}"
                )
            };
            let error = parser::parse(&source, high).unwrap_err();
            assert!(error.contains("line"), "{source}: {error}");
        }
    }
}

#[test]
fn enum_payload_types_use_the_existing_nesting_limit() {
    let ty = format!("{}i64{}", "List[".repeat(70), "]".repeat(70));
    for high in [true, false] {
        let source = if high {
            format!("enum Error:\n    Payload(value: {ty})\n")
        } else {
            format!("enum Error {{ Payload(value: {ty}); }}")
        };
        assert!(parser::parse(&source, high).unwrap_err().contains("64"));
    }
}

#[test]
fn contextual_enum_word_remains_an_identifier_inside_declarations_and_calls() {
    for high in [true, false] {
        let source = if high {
            "enum enum:\n    enum(enum: i64)\ndef enum(enum: enum) -> enum:\n    return enum\n"
        } else {
            "enum enum { enum(enum: i64); }\nfn enum(enum: enum) -> enum { return enum; }\n"
        };
        let program = parser::parse(source, high).unwrap();
        assert_eq!(program.enums[0].name, "enum");
        assert_eq!(program.enums[0].variants[0].name, "enum");
        assert_eq!(program.enums[0].variants[0].fields[0].0, "enum");
        assert_eq!(program.functions[0].name, "enum");
        assert_eq!(program.functions[0].params[0].0, "enum");
    }
}
