use nagic::{ast::*, check, emit, lexer, parser};

fn checked(source: &str, high: bool) -> Program {
    let mut program = parser::parse(source, high).unwrap();
    check::check(&mut program).unwrap_or_else(|error| panic!("{source}\n{error}"));
    program
}

#[test]
fn some_and_none_patterns_keep_binding_spans_without_synthetic_none_payload() {
    for high in [true, false] {
        let source = if high {
            "def number_or(value: i64?, fallback: i64) -> i64:\n    match value:\n        case Some(number):\n            return number\n        case None:\n            return fallback\n"
        } else {
            "fn number_or(value: i64?, fallback: i64) -> i64 {\n    match value {\n        case Some(number) { return number; }\n        case None { return fallback; }\n    }\n}\n"
        };
        let tokens = lexer::lex(source, high).unwrap();
        let mut program = checked(source, high);
        let S::Match(_, arms) = &mut program.functions[0].body[0].kind else {
            panic!("expected match");
        };
        let MatchPattern::Option {
            binding: Some(binding),
        } = &arms[0].pattern
        else {
            panic!("expected Some pattern");
        };
        assert_eq!(binding.name.as_deref(), Some("number"));
        assert_eq!(binding.span.end - binding.span.start, 1);
        assert_eq!(
            tokens[binding.span.start].kind,
            lexer::K::Id("number".into())
        );
        assert_eq!(binding.ty, Some(Type::named("i64")));
        assert_eq!(arms[0].pattern.bindings().len(), 1);
        assert!(matches!(
            &arms[1].pattern,
            MatchPattern::Option { binding: None }
        ));
        assert!(arms[1].pattern.bindings_mut().is_empty());
        let low = emit::low(&program);
        assert!(low.contains("case Some(number)"), "{low}");
        assert!(low.contains("case None {"), "{low}");
        let saved = checked(&low, false);
        let S::Match(_, saved_arms) = &saved.functions[0].body[0].kind else {
            panic!("expected saved match");
        };
        assert!(matches!(
            &saved_arms[1].pattern,
            MatchPattern::Option { binding: None }
        ));
    }
}

#[test]
fn option_matching_accepts_discarded_payloads_and_borrowed_string_contents() {
    let source = "def present(value: str?) -> bool:\n    match value:\n        case Some(_):\n            return True\n        case None:\n            return False\n\
                  def length_or_zero(text: str) -> i64:\n    match some(view(text)):\n        case Some(part):\n            return len(part)\n        case None:\n            return 0\n";
    let program = checked(source, true);
    let low = emit::low(&program);
    checked(&low, false);
    let S::Match(_, arms) = &program.functions[0].body[0].kind else {
        panic!("expected match");
    };
    assert!(matches!(&arms[0].pattern,
        MatchPattern::Option { binding: Some(binding) } if binding.name.is_none()));
    let S::Match(_, arms) = &program.functions[1].body[0].kind else {
        panic!("expected match");
    };
    assert_eq!(
        arms[0].pattern.bindings()[0].ty,
        Some(Type::generic("view", vec![Type::named("str")]))
    );
}

#[test]
fn option_cases_must_be_exhaustive_unique_and_match_the_subject_type() {
    for cases in [
        "        case Some(number):\n            return number\n",
        "        case None:\n            return 0\n",
        "        case Some(number):\n            return number\n        case Some(other):\n            return other\n        case None:\n            return 0\n",
        "        case Some(number):\n            return number\n        case None:\n            return 0\n        case None:\n            return 1\n",
        "        case Ok(number):\n            return number\n        case Err(_):\n            return 0\n",
    ] {
        let source = format!("def f(value: i64?) -> i64:\n    match value:\n{cases}");
        let mut program = parser::parse(&source, true).unwrap();
        assert!(check::check(&mut program).is_err(), "{source}");
    }
    let mut program = parser::parse("def f(value: Result[i64, Error]) -> i64:\n    match value:\n        case Some(number):\n            return number\n        case None:\n            return 0\n", true).unwrap();
    assert!(check::check(&mut program).is_err());
}

#[test]
fn some_patterns_reject_unsupported_payload_shapes_and_none_parentheses() {
    for high in [true, false] {
        for pattern in [
            "Some",
            "Some()",
            "Some(first, second)",
            "Some(Some(inner))",
            "None()",
            "None(value)",
            "_",
        ] {
            let source = if high {
                format!("def f(value: i64?):\n    match value:\n        case {pattern}:\n            return\n")
            } else {
                format!("fn f(value: i64?) {{ match value {{ case {pattern} {{ return; }} }} }}")
            };
            assert!(parser::parse(&source, high).is_err(), "{source}");
        }
    }
}

#[test]
fn option_payload_bindings_obey_move_scope_and_borrow_owner_rules() {
    for source in [
        "def take(text: str) -> i64:\n    return len(view(text))\n\
         def f(text: str) -> i64:\n    match some(view(text)):\n        case Some(part):\n            count = take(text)\n            return len(part) + count\n        case None:\n            return 0\n",
        "def f(value: i64?, number: i64) -> i64:\n    match value:\n        case Some(number):\n            return number\n        case None:\n            return 0\n",
        "def f(value: str?) -> i64:\n    match value:\n        case Some(text):\n            print(text)\n        case None:\n            print(\"none\")\n    match value:\n        case Some(text):\n            return len(view(text))\n        case None:\n            return 0\n",
    ] {
        let mut program = parser::parse(source, true).unwrap();
        assert!(check::check(&mut program).is_err(), "{source}");
    }
}
