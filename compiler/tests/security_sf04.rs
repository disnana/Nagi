//! Small typed-HTML business applications and exact checker-stage refusals.
//! Native Cargo remains the standard integration path; direct slices are recorded separately.
#[path = "support/native_triple.rs"]
mod native_triple;
use nagic::{check, emit, source};
use std::fs;

#[test]
fn list_detail_body_is_exact_in_three_source_forms() {
    let assertion = format!(
        "#[test] fn native_body() {{ let response = build_page().unwrap(); assert_eq!(response.status().value(), 200); assert_eq!(response.body(), {:?}.as_bytes()); }}",
        include_str!("fixtures/security-sf04/expected-list-detail.html")
    );
    native_triple::Fixture::new().run_three(
        "sf04-list-detail",
        include_str!("fixtures/security-sf04/list-detail.nagi"),
        include_str!("fixtures/security-sf04/list-detail-manual.low"),
        "",
        &assertion,
    );
}

#[test]
fn renamed_imports_build_the_page_and_keep_user_type_in_three_source_forms() {
    let assertion = format!(
        "#[test] fn native_body() {{ let response = build_page().unwrap(); assert_eq!(response.status().value(), 201); assert_eq!(response.body(), {:?}.as_bytes()); assert_eq!(user_value(), 7); }}",
        include_str!("fixtures/security-sf04/expected-renamed.html")
    );
    native_triple::Fixture::new().run_three(
        "sf04-renamed",
        include_str!("fixtures/security-sf04/renamed-page.nagi"),
        include_str!("fixtures/security-sf04/renamed-page-manual.low"),
        "",
        &assertion,
    );
}

#[test]
fn http_only_plain_response_keeps_user_type_in_three_source_forms() {
    native_triple::Fixture::new().run_three(
        "sf04-http-only",
        include_str!("fixtures/security-sf04/http-only.nagi"),
        include_str!("fixtures/security-sf04/http-only-manual.low"),
        "",
        "#[test] fn native_body() { let response = plain(); assert_eq!(response.status().value(), 200); assert_eq!(response.body(), b\"plain\"); assert_eq!(user_value(), 9); }",
    );
}

fn reject_checked(
    fixture: &native_triple::Fixture,
    name: &str,
    text: &str,
    reason: &str,
    marker: &str,
) {
    let lines: Vec<_> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(marker))
        .map(|(line, _)| line + 1)
        .collect();
    assert_eq!(lines.len(), 1, "one offending marker in {name}");
    fixture.write(name, text);
    let mut loaded = source::load(&fixture.0.join(name), name.ends_with(".nagi"))
        .unwrap_or_else(|error| panic!("not a semantic refusal: {name}: {error}"));
    let error = check::check(&mut loaded.program).expect_err("checker accepted a negative");
    let diagnostic = loaded.diagnostic(&error);
    assert!(diagnostic.contains(reason), "{name}: {diagnostic}");
    assert!(
        diagnostic.contains(&format!("{name}:{}\n", lines[0])),
        "wrong primary input line: {diagnostic}"
    );
}

#[test]
fn ownership_opacity_and_nominal_arguments_reject_at_checker_in_three_forms() {
    let fixture = native_triple::Fixture::new();
    for (case, high, manual, reason, marker) in [
        (
            "builtin-copy",
            include_str!("fixtures/security-sf04/builtin-copy.nagi"),
            include_str!("fixtures/security-sf04/builtin-copy-manual.low"),
            "Clone",
            "copy(view(fragment))",
        ),
        (
            "opaque-field",
            include_str!("fixtures/security-sf04/opaque-field.nagi"),
            include_str!("fixtures/security-sf04/opaque-field-manual.low"),
            "encoded",
            "fragment.encoded",
        ),
        (
            "use-after-consume",
            include_str!("fixtures/security-sf04/use-after-consume.nagi"),
            include_str!("fixtures/security-sf04/use-after-consume-manual.low"),
            "move",
            "return",
        ),
        (
            "borrowed-document",
            include_str!("fixtures/security-sf04/borrowed-document.nagi"),
            include_str!("fixtures/security-sf04/borrowed-document-manual.low"),
            "型",
            "return",
        ),
        (
            "nominal-document",
            include_str!("fixtures/security-sf04/nominal-document.nagi"),
            include_str!("fixtures/security-sf04/nominal-document-manual.low"),
            "型",
            "return",
        ),
        (
            "shared-document",
            include_str!("fixtures/security-sf04/shared-document.nagi"),
            include_str!("fixtures/security-sf04/shared-document-manual.low"),
            "型",
            "return",
        ),
    ] {
        // Only the existing parsed-program Low printer prepares rejected saved Low;
        // it is not claimed as successful checked CLI lowering of invalid High.
        let high_name = format!("{case}.nagi");
        fixture.write(&high_name, high);
        let parsed =
            source::load(&fixture.0.join(&high_name), true).expect("negative must parse/load");
        let saved = emit::low(&parsed.program);
        let high_path = fixture.0.join(&high_name);
        fs::remove_file(&high_path).unwrap();
        for (name, text) in [
            (high_name, high),
            (format!("{case}-saved.low"), saved.as_str()),
            (format!("{case}-manual.low"), manual),
        ] {
            if name.ends_with(".low") {
                assert!(!high_path.exists(), "saved/manual Low must be independent");
            }
            reject_checked(&fixture, &name, text, reason, marker);
            if name.ends_with(".nagi") {
                fs::remove_file(&high_path).unwrap();
            }
        }
    }
}
