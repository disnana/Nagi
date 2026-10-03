use nagic::{check, emit, parser};

fn check_source(source: &str, high: bool) -> Result<(), String> {
    let mut program = parser::parse(source, high).unwrap();
    check::check(&mut program)
}

#[test]
fn get_handlers_cannot_replace_builtin_endpoints() {
    for path in ["/health", "/stream", "/ws"] {
        for (source, high) in [
            (format!("@get(\"{path}\")\nasync def custom() -> Result[i64, Error]:\n    return ok(1)\n"), true),
            (format!("@get(\"{path}\")\nasync fn custom() -> Result[i64, Error] {{ return ok(1); }}\n"), false),
        ] {
            let error = check_source(&source, high).unwrap_err();
            assert!(error.contains("line 2:"), "{error}");
            assert!(error.contains(&format!("GET {path}は組み込みHTTP endpointです")), "{error}");
        }
    }
}

#[test]
fn equivalent_capture_paths_are_rejected_even_for_different_methods() {
    for method in ["get", "post"] {
        for (left, right) in [
            ("/items/{id}", "/items/{key}"),
            ("/items/{ba}}r}", "/items/{id}"),
            ("/items/{id}/details/{part}", "/items/{key}/details/{part}"),
            ("/file-{id}", "/file-{key}"),
            ("/files/{*tail}", "/files/{*rest}"),
        ] {
            for (source, high) in [
                (format!("@get(\"{left}\")\nasync def first() -> Result[i64, Error]:\n    return ok(1)\n@{method}(\"{right}\")\nasync def second() -> Result[i64, Error]:\n    return ok(2)\n"), true),
                (format!("@get(\"{left}\")\nasync fn first() -> Result[i64, Error] {{ return ok(1); }}\n@{method}(\"{right}\")\nasync fn second() -> Result[i64, Error] {{ return ok(2); }}\n"), false),
            ] {
                let error = check_source(&source, high).unwrap_err();
                assert!(error.contains("HTTPのpathが競合しています"), "{error}");
                assert!(error.contains(left) && error.contains(right), "{error}");
            }
        }
    }
}

#[test]
fn shared_patterns_distinct_paths_and_literal_braces_remain_available() {
    let source =
        "@get(\"/items/{id}\")\nasync def get_item() -> Result[i64, Error]:\n    return ok(1)\n\
        @post(\"/items/{id}\")\nasync def post_item() -> Result[i64, Error]:\n    return ok(2)\n\
        @get(\"/items/list\")\nasync def list_items() -> Result[i64, Error]:\n    return ok(3)\n\
        @get(\"/files/{*tail}\")\nasync def rest() -> Result[i64, Error]:\n    return ok(4)\n\
        @get(\"/literal/{{one}}\")\nasync def one() -> Result[i64, Error]:\n    return ok(5)\n\
        @get(\"/literal/{{two}}\")\nasync def two() -> Result[i64, Error]:\n    return ok(6)\n\
        @post(\"/health\")\nasync def post_health() -> Result[i64, Error]:\n    return ok(7)\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    check_source(&emit::low(&high), false).unwrap();
}
