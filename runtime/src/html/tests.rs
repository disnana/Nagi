use super::*;
use crate::ErrorKind;

fn invalid<T>(result: Result<T, Error>) {
    match result {
        Err(error) => assert!(matches!(error.kind, ErrorKind::Invalid)),
        Ok(_) => panic!("expected Invalid"),
    }
}
fn configured(output: i64, input: i64, nodes: i64, depth: i64, urls: i64) -> HtmlPolicy {
    limits(
        policy("https://example.invalid").unwrap(),
        output,
        input,
        nodes,
        depth,
        urls,
        2048,
    )
    .unwrap()
}
fn p() -> HtmlPolicy {
    policy("https://example.invalid").unwrap()
}

#[test]
fn text_attributes_and_document_title_keep_fixed_contexts() {
    let p = p();
    let attrs = title(attributes(&p), "Tea & \"café\"").unwrap();
    let body = element(&p, HtmlTag::P, attrs, text(&p, "literal &amp; é").unwrap()).unwrap();
    let doc = document(&p, "Menu & Tea", body).unwrap();
    assert_eq!(doc._encoded, "<!doctype html><html><head><meta charset=\"utf-8\"><title>Menu &amp; Tea</title></head><body><p title=\"Tea &amp; &quot;café&quot;\">literal &amp;amp; é</p></body></html>");
}
#[test]
fn join_and_explicit_copy_do_not_reencode() {
    let p = p();
    let text = text(&p, "A & B").unwrap();
    let duplicated = copy_fragment(&p, &text).unwrap();
    let joined = join(&p, text, duplicated).unwrap();
    assert_eq!(joined.encoded, "A &amp; BA &amp; B");
    assert_eq!(joined.summary.input, 10);
    assert_eq!(joined.summary.nodes, 2);
}
#[test]
fn attribute_order_is_stable_and_duplicates_are_rejected() {
    let p = p();
    let attrs = href(attributes(&p), navigation(&p, "/items/7?x=1&y=2").unwrap()).unwrap();
    let attrs = title(attrs, "List").unwrap();
    let a = element(&p, HtmlTag::A, attrs, text(&p, "Item").unwrap()).unwrap();
    assert_eq!(
        a.encoded,
        "<a title=\"List\" href=\"/items/7?x=1&amp;y=2\">Item</a>"
    );
    invalid(title(title(attributes(&p), "One").unwrap(), "Two"));
    invalid(href(
        href(attributes(&p), navigation(&p, "/one").unwrap()).unwrap(),
        navigation(&p, "/two").unwrap(),
    ));
}
#[test]
fn closed_content_models_reject_wrong_children() {
    let p = p();
    invalid(element(
        &p,
        HtmlTag::UL,
        attributes(&p),
        text(&p, " ").unwrap(),
    ));
    let block = element(&p, HtmlTag::DIV, attributes(&p), empty(&p)).unwrap();
    invalid(element(&p, HtmlTag::P, attributes(&p), block));
    let li = element(&p, HtmlTag::LI, attributes(&p), text(&p, "Item").unwrap()).unwrap();
    invalid(document(&p, "List", li));
    let li = element(&p, HtmlTag::LI, attributes(&p), text(&p, "Item").unwrap()).unwrap();
    let ul = element(&p, HtmlTag::UL, attributes(&p), li).unwrap();
    assert_eq!(ul.encoded, "<ul><li>Item</li></ul>");
}
#[test]
fn nested_anchor_void_children_and_href_on_other_tags_are_rejected() {
    let p = p();
    let child = element(&p, HtmlTag::A, attributes(&p), text(&p, "Item").unwrap()).unwrap();
    let wrapper = element(&p, HtmlTag::SPAN, attributes(&p), child).unwrap();
    invalid(element(&p, HtmlTag::A, attributes(&p), wrapper));
    invalid(element(
        &p,
        HtmlTag::BR,
        attributes(&p),
        text(&p, "").unwrap(),
    ));
    let attrs = href(attributes(&p), navigation(&p, "/items").unwrap()).unwrap();
    invalid(element(&p, HtmlTag::DIV, attrs, empty(&p)));
    assert_eq!(
        element(&p, HtmlTag::BR, attributes(&p), empty(&p))
            .unwrap()
            .encoded,
        "<br>"
    );
}
#[test]
fn equal_profiles_compose_but_different_limits_and_origins_do_not() {
    let a = policy("https://EXAMPLE.invalid:443/").unwrap();
    let b = p();
    assert!(join(&a, text(&a, "A").unwrap(), text(&b, "B").unwrap()).is_ok());
    let limited = configured(128, 8, 8, 8, 1);
    invalid(copy_fragment(&limited, &text(&b, "Item").unwrap()));
    let other = policy("https://other.invalid").unwrap();
    invalid(href(attributes(&b), navigation(&other, "/item").unwrap()));
}
#[test]
fn text_budget_counts_utf8_and_encoded_bytes() {
    let p = configured(5, 2, 1, 1, 0);
    assert_eq!(text(&p, "é").unwrap().encoded, "é");
    assert_eq!(text(&p, "&").unwrap().encoded, "&amp;");
    invalid(text(&p, "éa"));
    invalid(text(&p, "\""));
    let smaller = configured(1, 2, 1, 1, 0);
    invalid(text(&smaller, "é"));
}
#[test]
fn copy_join_charges_input_nodes_and_output_again() {
    let p = configured(10, 1, 2, 1, 0);
    let a = text(&p, "&").unwrap();
    let b = copy_fragment(&p, &a).unwrap();
    invalid(join(&p, a, b));
    let p = configured(9, 2, 2, 1, 0);
    let a = text(&p, "&").unwrap();
    let b = copy_fragment(&p, &a).unwrap();
    invalid(join(&p, a, b));
    let p = configured(10, 2, 1, 1, 0);
    let a = text(&p, "&").unwrap();
    let b = copy_fragment(&p, &a).unwrap();
    invalid(join(&p, a, b));
    let p = configured(10, 2, 2, 1, 0);
    let a = text(&p, "&").unwrap();
    let b = copy_fragment(&p, &a).unwrap();
    assert!(join(&p, a, b).is_ok());
}
#[test]
fn document_charges_seven_fixed_nodes_and_wrapper_depth() {
    let too_few = configured(256, 16, 6, 3, 0);
    invalid(document(&too_few, "List", empty(&too_few)));
    let exact = configured(256, 16, 7, 3, 0);
    assert!(document(&exact, "List", empty(&exact)).is_ok());
    let shallow = configured(256, 16, 7, 2, 0);
    invalid(document(&shallow, "List", empty(&shallow)));
    let p = configured(256, 16, 7, 3, 0);
    invalid(document(&p, "List", text(&p, "").unwrap()));
}
#[test]
fn document_output_includes_title_and_full_wrapper() {
    let expected = "<!doctype html><html><head><meta charset=\"utf-8\"><title>A &amp; B</title></head><body></body></html>";
    let n = expected.len() as i64;
    let below = configured(n - 1, 5, 7, 3, 0);
    invalid(document(&below, "A & B", empty(&below)));
    let exact = configured(n, 5, 7, 3, 0);
    assert_eq!(
        document(&exact, "A & B", empty(&exact)).unwrap()._encoded,
        expected
    );
    let above = configured(n + 1, 5, 7, 3, 0);
    assert!(document(&above, "A & B", empty(&above)).is_ok());
}
#[test]
fn attribute_input_and_prefix_bytes_are_aggregated() {
    let p = configured(9, 16, 8, 8, 1);
    invalid(title(attributes(&p), "A"));
    let p = configured(128, 2, 8, 8, 1);
    let attrs = title(attributes(&p), "A").unwrap();
    invalid(element(&p, HtmlTag::P, attrs, text(&p, "BC").unwrap()));
    let p = configured(128, 3, 8, 8, 1);
    let attrs = title(attributes(&p), "A").unwrap();
    assert!(element(&p, HtmlTag::P, attrs, text(&p, "BC").unwrap()).is_ok());
}
#[test]
fn element_depth_and_href_count_are_bounded() {
    let p = configured(256, 64, 16, 2, 1);
    let one = element(&p, HtmlTag::SPAN, attributes(&p), empty(&p)).unwrap();
    let two = element(&p, HtmlTag::SPAN, attributes(&p), one).unwrap();
    invalid(element(&p, HtmlTag::SPAN, attributes(&p), two));
    let a = element(
        &p,
        HtmlTag::A,
        href(attributes(&p), navigation(&p, "/one").unwrap()).unwrap(),
        empty(&p),
    )
    .unwrap();
    let b = element(
        &p,
        HtmlTag::A,
        href(attributes(&p), navigation(&p, "/two").unwrap()).unwrap(),
        empty(&p),
    )
    .unwrap();
    invalid(join(&p, a, b));
    let no_links = configured(256, 64, 16, 2, 0);
    invalid(navigation(&no_links, "/one"));
}
#[test]
fn limit_changes_cannot_reset_or_raise_profile_quotas() {
    invalid(limits(p(), 0, 16, 8, 3, 1, 2048));
    invalid(limits(p(), 256, -1, 8, 3, 1, 2048));
    invalid(limits(p(), 65537, 16, 8, 3, 1, 2048));
    invalid(limits(configured(128, 8, 8, 3, 1), 129, 8, 8, 3, 1, 2048));
    invalid(limits(p(), 256, 16, 8, 3, 1, 1));
}
#[test]
fn navigation_keeps_ordinary_canonical_meaning_and_empty_parts() {
    let p = policy("https://BÜCHER.example.invalid:443/").unwrap();
    assert_eq!(
        p.profile.base.host_str(),
        Some("xn--bcher-kva.example.invalid")
    );
    for value in [
        "/items/7?view=details#top",
        "/items/7",
        "/items/7?#",
        "/café",
    ] {
        let n = navigation(&p, value).unwrap();
        let expected = p.profile.base.join(value).unwrap();
        let result = p.profile.base.join(&n.value).unwrap();
        assert!(exactly_one_slash(&n.value));
        assert!(same_destination(&expected, &result));
    }
    assert_eq!(navigation(&p, "/items/7?#").unwrap().value, "/items/7?#");
    assert!(policy("https://127.0.0.1/").is_ok());
    assert!(policy("https://[::1]/").is_ok());
}
#[test]
fn navigation_and_base_reject_unsupported_profiles_and_small_url_bounds() {
    let p = p();
    for value in [
        "items/7",
        "https://example.invalid/items",
        "/a b",
        "/a\tb",
        "/a\\b",
        "/a\u{7f}",
    ] {
        invalid(navigation(&p, value));
    }
    for base in [
        "http://example.invalid",
        "https://example.invalid./",
        "https://example.invalid/items",
        "https://example.invalid?",
        "https://example.invalid#",
        "https://user@example.invalid",
        "https://@example.invalid",
    ] {
        invalid(policy(base));
    }
    let p = limits(p, 128, 64, 8, 3, 1, 32).unwrap();
    invalid(navigation(&p, format!("/{}", "a".repeat(32)).as_str()));
    // A bounded normal Unicode path expands during URL percent encoding.
    let path = format!("/{}", "é".repeat(6));
    invalid(navigation(&p, &path));
}
#[test]
fn final_serialized_href_gate_is_separate_from_raw_input_check() {
    assert!(exactly_one_slash("/items/7"));
    assert!(!exactly_one_slash(""));
    assert!(!exactly_one_slash("//"));
    let base = Url::parse("https://example.invalid/").unwrap();
    let absent = base.join("/items").unwrap();
    let empty = base.join("/items?#").unwrap();
    assert!(!same_destination(&absent, &empty));
    assert!(!same_destination(&absent, &base.join("/other").unwrap()));
    assert!(!same_destination(
        &absent,
        &Url::parse("https://other.invalid/items").unwrap()
    ));
}
#[test]
fn encoder_required_characters_and_unicode_match_native_error_boundary() {
    let input = "&<>\"'é\u{301}";
    let output = "&amp;&lt;&gt;&quot;&#39;é\u{301}";
    assert_eq!(
        encoding::encode(input, input.len(), output.len()).unwrap(),
        output
    );
    assert_eq!(
        encoding::encoded_len(input, input.len(), output.len()).unwrap(),
        output.len()
    );
}
#[test]
fn encoder_input_and_output_caps_are_distinct_byte_budgets() {
    assert_eq!(encoding::encode("é", 2, 2).unwrap(), "é");
    invalid(encoding::encode("é", 1, 2));
    invalid(encoding::encode("é", 2, 1));
    assert_eq!(encoding::encode("&", 1, 5).unwrap(), "&amp;");
    invalid(encoding::encode("&", 1, 4));
}
#[test]
fn encoder_control_policy_preserves_allowed_references_and_c1() {
    assert_eq!(
        encoding::encode("\t\n\r\u{85}", 5, 16).unwrap(),
        "&#9;&#10;&#13;\u{85}"
    );
    for c in (0..=31)
        .chain(std::iter::once(127))
        .filter(|c| ![9, 10, 13].contains(c))
    {
        invalid(encoding::encode(
            &char::from_u32(c).unwrap().to_string(),
            8,
            32,
        ));
    }
}
#[test]
fn invalid_error_messages_do_not_echo_dynamic_content() {
    let sentinel = "private-item\0";
    let error = match text(&p(), sentinel) {
        Err(error) => error,
        Ok(_) => panic!("expected Invalid"),
    };
    assert!(matches!(error.kind, ErrorKind::Invalid));
    assert!(!error.message.contains("private-item"));
}
