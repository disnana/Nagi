use super::*;
use crate::html;

const EXPECTED_CSP: &str = "default-src 'none'; script-src 'none'; style-src 'none'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'";
const EXPECTED_DOCUMENT: &[u8] = b"<!doctype html><html><head><meta charset=\"utf-8\"><title>Items</title></head><body><p>Item</p></body></html>";

fn document() -> html::HtmlDocument {
    let policy = html::policy("https://example.invalid").unwrap();
    let body = html::element(
        &policy,
        html::HtmlTag::P,
        html::attributes(&policy),
        html::text(&policy, "Item").unwrap(),
    )
    .unwrap();
    html::document(&policy, "Items", body).unwrap()
}
fn managed(response: &axum::http::Response<BufferedBody>) {
    assert_eq!(response.headers()["content-security-policy"], EXPECTED_CSP);
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
}

#[test]
fn html_response_consumes_a_checked_document_and_keeps_debug_opaque() {
    let request = response_public::tests::request_fixture(256);
    let context = response_context(&request);
    let response = html_response(context, Status::CREATED, document()).unwrap();
    assert_eq!(response.body(), EXPECTED_DOCUMENT);
    assert!(!format!("{response:?}").contains("Items"));
    let response = response.into_http(false);
    managed(&response);
    assert_eq!(response.status(), Status::CREATED.0);
    assert_eq!(
        response.headers()[names::CONTENT_TYPE],
        "text/html; charset=utf-8"
    );
    assert_eq!(response.body().bytes.as_ref(), EXPECTED_DOCUMENT);
}

#[test]
fn content_encoding_is_reserved_for_html_only() {
    let request = response_public::tests::request_fixture(256);
    let context = response_context(&request);
    for name in ["Content-Encoding", "CONTENT-ENCODING", "content-encoding"] {
        let error = append_header_text(
            html_response(context, Status::OK, document()).unwrap(),
            name,
            "gzip",
        )
        .unwrap_err();
        assert!(matches!(error.kind, ErrorKind::Invalid));
    }
    for response in [
        empty(context, Status::OK).unwrap(),
        text(context, Status::OK, "Item").unwrap(),
        bytes(context, Status::OK, b"Item").unwrap(),
        json(context, Status::OK, &vec![1]).unwrap(),
    ] {
        let response = append_header_text(
            append_header_text(response, "Content-Encoding", "gzip").unwrap(),
            "Content-Encoding",
            "br",
        )
        .unwrap()
        .into_http(false);
        managed(&response);
        assert_eq!(
            response
                .headers()
                .get_all(names::CONTENT_ENCODING)
                .iter()
                .map(HeaderValue::as_bytes)
                .collect::<Vec<_>>(),
            [b"gzip".as_slice(), b"br".as_slice()]
        );
    }
}

#[test]
fn finalizer_rejects_an_internal_html_encoding_invariant_violation() {
    let request = response_public::tests::request_fixture(256);
    let mut response =
        html_response(response_context(&request), Status::CREATED, document()).unwrap();
    // Private invariant injection, not a public factory or network request.
    response
        .produced
        .headers
        .append(b"content-encoding", b"gzip")
        .unwrap();
    response
        .produced
        .headers
        .append(b"x-trace", b"discard")
        .unwrap();
    let response = response.into_http(false);
    managed(&response);
    assert_eq!(response.status(), Status::INTERNAL_SERVER_ERROR.0);
    assert_eq!(
        response.headers()[names::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    assert_eq!(response.body().bytes.as_ref(), b"internal error");
    assert!(!response.headers().contains_key(names::CONTENT_ENCODING));
    assert!(!response.headers().contains_key("x-trace"));
}

#[test]
fn typed_head_and_bodyless_statuses_follow_owned_representation_rules() {
    let request = response_public::tests::request_fixture(256);
    let context = response_context(&request);
    let response = html_response(context, Status::OK, document())
        .unwrap()
        .into_http(true);
    managed(&response);
    assert!(response.body().bytes.is_empty());
    assert_eq!(
        response.headers()[names::CONTENT_LENGTH].to_str().unwrap(),
        EXPECTED_DOCUMENT.len().to_string()
    );
    assert_eq!(
        response.headers()[names::CONTENT_TYPE],
        "text/html; charset=utf-8"
    );
    for head in [false, true] {
        for status in [
            Status::NO_CONTENT,
            Status::RESET_CONTENT,
            Status::NOT_MODIFIED,
        ] {
            let response = html_response(context, status, document())
                .unwrap()
                .into_http(head);
            managed(&response);
            assert!(response.body().bytes.is_empty());
            assert!(!response.headers().contains_key(names::CONTENT_TYPE));
            assert!(!response.headers().contains_key(names::CONTENT_ENCODING));
            assert!(!response.headers().contains_key(names::TRANSFER_ENCODING));
            assert_eq!(
                response
                    .headers()
                    .get(names::CONTENT_LENGTH)
                    .map(HeaderValue::as_bytes),
                if status == Status::RESET_CONTENT {
                    Some(b"0".as_slice())
                } else {
                    None
                }
            );
        }
    }
    // HEAD/empty statuses are selected only after ordinary document budgeting.
    let policy = html::limits(
        html::policy("https://example.invalid").unwrap(),
        64,
        32,
        8,
        3,
        0,
        64,
    )
    .unwrap();
    assert!(html::document(&policy, "Items", html::empty(&policy)).is_err());
}

#[test]
fn an_ordinary_html_business_mapper_uses_the_standard_finalizer() {
    fn mapper(context: &ResponseContext, _: i64) -> Result<Response, Error> {
        html_response(context, Status::BAD_REQUEST, document())
    }
    let request = response_public::tests::request_fixture(256);
    let response = (app((), mapper).mapper)(response_context(&request), 7)
        .unwrap()
        .into_http(false);
    managed(&response);
    assert_eq!(response.status(), Status::BAD_REQUEST.0);
    assert_eq!(response.body().bytes.as_ref(), EXPECTED_DOCUMENT);
}
