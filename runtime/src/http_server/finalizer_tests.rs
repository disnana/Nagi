use super::*;

const EXPECTED_CSP: &str = "default-src 'none'; script-src 'none'; style-src 'none'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'";

fn managed(response: &axum::http::Response<BufferedBody>) {
    assert_eq!(response.headers()["content-security-policy"], EXPECTED_CSP);
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
}

#[test]
fn standard_kinds_have_fixed_mime_and_managed_security_headers() {
    for (response, mime) in [
        (empty(Status::OK), None),
        (text(Status::OK, "Item"), Some("text/plain; charset=utf-8")),
        (bytes(Status::OK, b"Item"), Some("application/octet-stream")),
        (
            json(Status::OK, &vec![1, 2]).unwrap(),
            Some("application/json"),
        ),
    ] {
        let response = response.into_http(false);
        managed(&response);
        assert_eq!(
            response
                .headers()
                .get(names::CONTENT_TYPE)
                .map(|v| v.to_str().unwrap()),
            mime
        );
    }
}

#[test]
fn managed_values_are_reasserted_from_owned_representation() {
    let mut response = text(Status::CREATED, "Item");
    // Ordinary stale internal values must not replace the owned classification.
    response.headers.insert(
        names::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response.headers.insert(
        "content-security-policy",
        HeaderValue::from_static("default-src 'none'"),
    );
    response
        .headers
        .insert("x-content-type-options", HeaderValue::from_static("none"));
    let response = response.into_http(false);
    managed(&response);
    assert_eq!(
        response.headers()[names::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    assert_eq!(response.body().bytes.as_ref(), b"Item");
}

#[test]
fn head_and_bodyless_statuses_retain_security_headers() {
    let response = text(Status::OK, "Item").into_http(true);
    managed(&response);
    assert!(response.body().bytes.is_empty());
    assert_eq!(response.headers()[names::CONTENT_LENGTH], "4");
    assert_eq!(
        response.headers()[names::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    for status in [
        Status::NO_CONTENT,
        Status::RESET_CONTENT,
        Status::NOT_MODIFIED,
    ] {
        let response = append_header_text(text(status, "Item"), "Content-Encoding", "gzip")
            .unwrap()
            .into_http(false);
        managed(&response);
        assert!(response.body().bytes.is_empty());
        assert!(!response.headers().contains_key(names::CONTENT_TYPE));
        assert!(!response.headers().contains_key(names::CONTENT_ENCODING));
        assert!(!response.headers().contains_key(names::TRANSFER_ENCODING));
        if status == Status::RESET_CONTENT {
            assert_eq!(response.headers()[names::CONTENT_LENGTH], "0");
        } else {
            assert!(!response.headers().contains_key(names::CONTENT_LENGTH));
        }
    }
}

#[test]
fn builtins_and_ordinary_error_mapper_use_the_same_finalizer() {
    for status in [
        Status::NOT_FOUND,
        Status::BAD_REQUEST,
        Status::SERVICE_UNAVAILABLE,
        Status::GATEWAY_TIMEOUT,
    ] {
        let response = transport(status, false, true);
        managed(&response);
        assert_eq!(response.status(), status.0);
        assert_eq!(
            response.headers()[names::CONTENT_TYPE],
            "text/plain; charset=utf-8"
        );
    }
    for (source, challenge, vary) in [
        (PolicySource::Bearer, "Bearer", "Authorization"),
        (PolicySource::Session, "Session", "Cookie"),
    ] {
        for (failure, status) in [
            (Failure::invalid_credential(), Status::UNAUTHORIZED),
            (Failure::expired(), Status::UNAUTHORIZED),
            (Failure::denied(), Status::FORBIDDEN),
            (Failure::invalid_request(), Status::BAD_REQUEST),
            (Failure::unavailable(), Status::SERVICE_UNAVAILABLE),
            (Failure::internal(), Status::INTERNAL_SERVER_ERROR),
        ] {
            let response = finalize_authenticated(
                security_response(failure, source).into_http(false),
                source,
                false,
            );
            managed(&response);
            assert_eq!(response.status(), status.0);
            assert_eq!(response.headers()[names::CACHE_CONTROL], "no-store");
            assert_eq!(response.headers()[names::VARY], vary);
            assert_eq!(
                response.headers()[names::CONTENT_TYPE],
                "text/plain; charset=utf-8"
            );
            assert_eq!(
                response
                    .headers()
                    .get(names::WWW_AUTHENTICATE)
                    .map(|value| value.to_str().unwrap()),
                (status == Status::UNAUTHORIZED).then_some(challenge)
            );
        }
    }
    let response = default_error(Error::internal("private detail")).into_http(false);
    managed(&response);
    assert_eq!(response.status(), Status::INTERNAL_SERVER_ERROR.0);
    assert_eq!(response.body().bytes.as_ref(), b"internal error");
}
