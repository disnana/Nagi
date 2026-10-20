use super::*;

const EXPECTED_CSP: &str = "default-src 'none'; script-src 'none'; style-src 'none'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'";

fn managed(response: &HttpResponse) {
    assert_eq!(response.headers()["content-security-policy"], EXPECTED_CSP);
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
}

#[test]
fn standard_kinds_have_fixed_mime_and_managed_security_headers() {
    let request = response_public::tests::request_fixture(16);
    let context = response_context(&request);
    for (response, mime) in [
        (empty(context, Status::OK).unwrap(), None),
        (
            text(context, Status::OK, "Item").unwrap(),
            Some("text/plain; charset=utf-8"),
        ),
        (
            bytes(context, Status::OK, b"Item").unwrap(),
            Some("application/octet-stream"),
        ),
        (
            json(context, Status::OK, &vec![1, 2]).unwrap(),
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
    let request = response_public::tests::request_fixture(16);
    let mut response = text(response_context(&request), Status::CREATED, "Item").unwrap();
    // Ordinary stale internal values must not replace the owned classification.
    response
        .produced
        .headers
        .append(b"content-type", b"application/json")
        .unwrap();
    response
        .produced
        .headers
        .append(b"content-security-policy", b"default-src 'none'")
        .unwrap();
    response
        .produced
        .headers
        .append(b"x-content-type-options", b"none")
        .unwrap();
    for (name, value) in [
        (b"Content-Length".as_slice(), b"999".as_slice()),
        (b"TRANSFER-ENCODING", b"chunked"),
        (b"aCcEsS-CoNtRoL-AlLoW-OrIgIn", b"https://stale.invalid"),
        (b"Set-Cookie", b"stale=value"),
    ] {
        response.produced.headers.append(name, value).unwrap();
    }
    let response = response.into_http(false);
    managed(&response);
    for name in [
        "content-type",
        "content-security-policy",
        "x-content-type-options",
    ] {
        assert_eq!(response.headers().get_all(name).iter().count(), 1);
    }
    for name in [
        "content-length",
        "transfer-encoding",
        "access-control-allow-origin",
        "set-cookie",
    ] {
        assert!(!response.headers().contains_key(name));
    }
    assert_eq!(
        response.headers()[names::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    assert_eq!(response.body().bytes.as_ref(), b"Item");
}

#[test]
fn head_and_bodyless_statuses_retain_security_headers() {
    let request = response_public::tests::request_fixture(16);
    let context = response_context(&request);
    let response = text(context, Status::OK, "Item").unwrap().into_http(true);
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
        let response = append_header_text(
            text(context, status, "Item").unwrap(),
            "Content-Encoding",
            "gzip",
        )
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
    let request = response_public::tests::request_fixture(64);
    let context = response_context(&request);
    for status in [
        Status::NOT_FOUND,
        Status::BAD_REQUEST,
        Status::SERVICE_UNAVAILABLE,
        Status::GATEWAY_TIMEOUT,
    ] {
        let response = transport(context, status, false, true);
        managed(&response);
        assert_eq!(response.status(), status.0);
        assert_eq!(
            response.headers()[names::CONTENT_TYPE],
            "text/plain; charset=utf-8"
        );
    }
    for (source, challenge, vary) in [
        (PolicySource::Bearer, "Bearer", "Authorization, Cookie"),
        (PolicySource::Session, "Session", "Authorization, Cookie"),
    ] {
        for (failure, status) in [
            (Failure::invalid_credential(), Status::UNAUTHORIZED),
            (Failure::expired(), Status::UNAUTHORIZED),
            (Failure::denied(), Status::FORBIDDEN),
            (Failure::invalid_request(), Status::BAD_REQUEST),
            (Failure::unavailable(), Status::SERVICE_UNAVAILABLE),
            (Failure::internal(), Status::INTERNAL_SERVER_ERROR),
        ] {
            let response = vary::finish(finalize_authenticated(
                security_response(context, failure, source, false),
                source,
                false,
            ));
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
    let response = default_error(context, Error::internal("private detail"))
        .unwrap()
        .into_http(false);
    managed(&response);
    assert_eq!(response.status(), Status::INTERNAL_SERVER_ERROR.0);
    assert_eq!(response.body().bytes.as_ref(), b"internal error");
}
