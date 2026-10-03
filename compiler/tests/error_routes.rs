use nagic::{check, emit, parser};

#[test]
fn private_error_data_is_rejected_at_http_json_boundaries_in_high_and_low() {
    let declarations = [
        "class Private:\n    cause: Error\n",
        "enum Failure:\n    Missing\nclass Private:\n    cause: Failure\n",
    ];
    for declaration in declarations {
        for handler in [
            "@post(\"/input\")\nasync def input(body: Private) -> Result[str, Error]:\n    return ok(\"accepted\")\n",
            "@get(\"/output\")\nasync def output(value: Private) -> Result[Private, Error]:\n    return ok(value)\n",
            "@get(\"/output\")\nasync def output() -> Result[List[Private], Error]:\n    return ok([])\n",
            "@get(\"/output\")\nasync def output() -> Result[Private?, Error]:\n    return ok(None)\n",
        ] {
            let mut high = parser::parse(&format!("{declaration}{handler}"), true).unwrap();
            let error = check::check(&mut high).expect_err("private data reached HTTP JSON");
            assert!(error.contains("HTTP") && error.contains("JSON"), "{error}");
            let mut low = parser::parse(&emit::low(&high), false).unwrap();
            let error = check::check(&mut low).expect_err("Low exposed private data through HTTP");
            assert!(error.contains("HTTP") && error.contains("JSON"), "{error}");
        }
    }
    let mut direct_enum = parser::parse(
        "enum Failure:\n    Missing\n@get(\"/output\")\nasync def output() -> Result[Failure, Error]:\n    return ok(Failure.Missing)\n",
        true,
    )
    .unwrap();
    let error = check::check(&mut direct_enum).unwrap_err();
    assert!(error.contains("HTTP") && error.contains("JSON"), "{error}");
}

#[test]
fn existing_data_and_html_http_routes_keep_their_representations() {
    let source = "class Input:\n    id: i64\nclass Output:\n    id: i64\n@post(\"/data\")\nasync def data(body: Input) -> Result[Output, Error]:\n    return ok(Output(id=body.id))\n@get(\"/optional\")\nasync def optional() -> Result[Output?, Error]:\n    return ok(None)\n@get(\"/page\")\nasync def page() -> Result[Html, Error]:\n    return ok(html(\"<p>Hello</p>\"))\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let rust = emit::rust(&low).unwrap();
    assert!(rust.contains("serde::Serialize"));
    assert!(rust.contains("__route_"));
}
