use super::*;
fn policy() -> CorsPolicy {
    cors_policy(
        vec!["https://app.example.invalid".into()],
        vec!["content-type".into(), "x-csrf-token".into()],
        CorsCredentials::ALLOW,
        60,
        4,
        256,
        4,
        128,
    )
    .unwrap()
}
fn headers(origin: &str) -> HeaderMap {
    let mut map = HeaderMap::new();
    map.insert(names::ORIGIN, HeaderValue::from_str(origin).unwrap());
    map
}
#[test]
fn exact_origins_are_canonical_and_security_wrapper_remains_https_only() {
    for (raw, canonical) in [
        (
            "https://APP.example.invalid:443",
            "https://app.example.invalid",
        ),
        (
            "http://app.example.invalid:80",
            "http://app.example.invalid",
        ),
        ("https://[0:0:0:0:0:0:0:1]:443", "https://[::1]"),
    ] {
        assert_eq!(Origin::checked(raw).unwrap().serialized(), canonical);
    }
    for raw in [
        "null",
        "*",
        "https://app.example.invalid/",
        "https://user@app.example.invalid",
        "https://app.example.invalid%20",
        "https://app.example.invalid?q=1",
        "https://app.example.invalid#f",
        "https://app.example.invalid.",
        "https://日本.invalid",
        "https://app.example.invalid:0",
        "https://app.example.invalid:0443",
        "https://app.example.invalid,https://other.invalid",
        "https://app.example.invalid https://other.invalid",
    ] {
        assert!(Origin::checked(raw).is_err());
    }
    assert!(!BoundOrigin::matches_serialized(
        "http://app.example.invalid",
        "http://app.example.invalid"
    ));
    assert!(!BoundOrigin::matches_serialized(
        "https://app.example.invalid",
        "http://app.example.invalid"
    ));
    assert!(BoundOrigin::matches_serialized(
        "https://app.example.invalid",
        "https://APP.example.invalid:443"
    ));
    assert_ne!(
        origin::WireAuthority::checked("app.example.invalid").unwrap(),
        origin::WireAuthority::checked("app.example.invalid:443").unwrap()
    );
}
#[test]
fn constructor_limits_defaults_duplicates_and_max_age_are_checked() {
    for mode in [CorsCredentials::ALLOW, CorsCredentials::OMIT] {
        assert!(cors_policy(vec!["*".into()], vec![], mode, 0, 1, 64, 1, 16).is_err());
        assert!(cors_policy(vec![], vec![], mode, 0, 1, 64, 1, 16).is_ok());
    }
    for age in [-1, 4294967296] {
        assert!(cors_policy(vec![], vec![], CorsCredentials::OMIT, age, 1, 64, 1, 16).is_err());
    }
    for age in [0, 60, 4294967295] {
        assert!(cors_policy(vec![], vec![], CorsCredentials::OMIT, age, 1, 64, 1, 16).is_ok());
    }
    assert!(cors_policy(vec![], vec![], CorsCredentials::OMIT, 0, 0, 64, 1, 16).is_err());
    assert!(cors_policy(
        vec!["https://a.invalid".into()],
        vec!["ab".into(), "cd".into()],
        CorsCredentials::OMIT,
        0,
        1,
        17,
        2,
        6
    )
    .is_ok());
    assert!(cors_policy(
        vec!["https://a.invalid".into()],
        vec!["ab".into(), "cd".into()],
        CorsCredentials::OMIT,
        0,
        1,
        16,
        2,
        6
    )
    .is_err());
    assert!(cors_policy(
        vec![],
        vec!["ab".into(), "cd".into()],
        CorsCredentials::OMIT,
        0,
        1,
        64,
        2,
        5
    )
    .is_err());
    assert!(cors_policy(
        vec!["https://a.invalid".into(), "https://A.invalid:443".into()],
        vec![],
        CorsCredentials::OMIT,
        0,
        2,
        64,
        1,
        16
    )
    .is_err());
    for list in [
        vec!["Content-Type".into(), "content-type".into()],
        vec!["*".into()],
        vec!["bad header".into()],
    ] {
        assert!(cors_policy(vec![], list, CorsCredentials::OMIT, 0, 1, 64, 2, 32).is_err());
    }
}
fn app() -> App<(), Error> {
    super::super::route(
        app_default(()),
        Method::GET,
        "/items/{id}",
        public_policy(),
        |request, _, _| async move { empty(response_context(&request), Status::OK) },
    )
    .unwrap()
}
#[test]
fn attachment_requires_exact_registered_record_and_one_owned_policy() {
    assert!(cors_route(app(), Method::GET, "/items/7", policy()).is_err());
    assert!(cors_route(app(), Method::HEAD, "/items/{id}", policy()).is_err());
    assert!(cors_route(app(), Method::POST, "/items/{id}", policy()).is_err());
    let configured = cors_route(app(), Method::GET, "/items/{id}", policy()).unwrap();
    assert!(configured.routes["/items/{id}"][0].cors.is_some());
    assert!(cors_route(configured, Method::GET, "/items/{id}", policy()).is_err());
    for method in [
        Method::CONNECT,
        Method::TRACE,
        Method::from_bytes(b"TRACK").unwrap(),
        Method::from_bytes(b"*").unwrap(),
    ] {
        assert!(cors_route(app(), method, "/items/{id}", policy()).is_err());
    }
}
#[test]
fn preflight_method_and_header_lists_have_correct_error_stage() {
    let mut map = headers("https://app.example.invalid");
    assert_eq!(requested_method(&map), Err(Status::BAD_REQUEST));
    map.insert(
        names::ACCESS_CONTROL_REQUEST_METHOD,
        HeaderValue::from_static("POST"),
    );
    assert_eq!(requested_method(&map).unwrap(), Method::POST);
    map.append(
        names::ACCESS_CONTROL_REQUEST_METHOD,
        HeaderValue::from_static("POST"),
    );
    assert_eq!(requested_method(&map), Err(Status::BAD_REQUEST));
    map.remove(names::ACCESS_CONTROL_REQUEST_METHOD);
    for method in ["*", "CONNECT", "TRACE", "TRACK", "POST GET", ""] {
        map.insert(
            names::ACCESS_CONTROL_REQUEST_METHOD,
            HeaderValue::from_str(method).unwrap(),
        );
        assert_eq!(requested_method(&map), Err(Status::BAD_REQUEST));
    }
    map.insert(
        names::ACCESS_CONTROL_REQUEST_HEADERS,
        HeaderValue::from_static(" X-CSRF-Token, Content-Type "),
    );
    assert_eq!(
        requested_headers(&map, 2, 64)
            .unwrap()
            .iter()
            .map(HeaderName::as_str)
            .collect::<Vec<_>>(),
        vec!["content-type", "x-csrf-token"]
    );
    for names in ["", "a,,b", "content-type, Content-Type", "*", "bad header"] {
        map.insert(
            names::ACCESS_CONTROL_REQUEST_HEADERS,
            HeaderValue::from_str(names).unwrap(),
        );
        assert_eq!(requested_headers(&map, 2, 64), Err(Status::BAD_REQUEST));
    }
    map.insert(
        names::ACCESS_CONTROL_REQUEST_HEADERS,
        HeaderValue::from_static("a,b"),
    );
    assert_eq!(requested_headers(&map, 1, 64), Err(Status::BAD_REQUEST));
    assert_eq!(requested_headers(&map, 2, 3), Err(Status::BAD_REQUEST));
}
#[test]
fn actual_grants_preserve_underlying_status_and_typed_dependency_union() {
    let configured = cors_route(app(), Method::GET, "/items/{id}", policy()).unwrap();
    let route = &configured.routes["/items/{id}"][0];
    let response_request = super::super::response_public::tests::request_fixture(16);
    let response_context = response_context(&response_request);
    for status in [
        Status::OK,
        Status::UNAUTHORIZED,
        Status::FORBIDDEN,
        Status::CONTENT_TOO_LARGE,
        Status::INTERNAL_SERVER_ERROR,
        Status::SERVICE_UNAVAILABLE,
        Status::GATEWAY_TIMEOUT,
        Status::NOT_MODIFIED,
    ] {
        let wire = Context::new(route, &headers("https://APP.example.invalid:443"), false)
            .finish(empty(response_context, status).unwrap().into_http(false));
        assert_eq!(wire.status(), status.0);
        assert_eq!(
            wire.headers()[names::ACCESS_CONTROL_ALLOW_ORIGIN],
            "https://app.example.invalid"
        );
        assert_eq!(
            wire.headers()[names::ACCESS_CONTROL_ALLOW_CREDENTIALS],
            "true"
        );
        assert_eq!(wire.headers()[names::VARY], "Origin");
    }
    for origin in [
        "null",
        "https://other.invalid",
        "http://app.example.invalid",
    ] {
        let wire = Context::new(route, &headers(origin), false).finish(
            text(response_context, Status::OK, "unchanged")
                .unwrap()
                .into_http(false),
        );
        assert_eq!(wire.status(), StatusCode::OK);
        assert!(!wire
            .headers()
            .contains_key(names::ACCESS_CONTROL_ALLOW_ORIGIN));
        assert_eq!(wire.headers()[names::VARY], "Origin");
    }
    let mut wire = transport(response_context, Status::INTERNAL_SERVER_ERROR, false, true);
    vary::add(&mut wire, vary::Dependencies::GUARDED);
    vary::add(&mut wire, vary::Dependencies::COOKIE);
    let wire = Context::new(route, &headers("https://app.example.invalid"), false).finish(wire);
    assert_eq!(
        wire.headers()[names::VARY],
        "Authorization, Cookie, Origin, X-CSRF-Token"
    );
    assert_eq!(wire.headers().get_all(names::VARY).iter().count(), 1);
    assert_eq!(wire.headers()[names::CACHE_CONTROL], "no-store");
    assert!(wire.headers().contains_key("content-security-policy"));
}
#[test]
fn preflight_context_has_only_metadata_dependencies_and_reserved_headers_stay_owned() {
    let response_request = super::super::response_public::tests::request_fixture(64);
    let response_context = response_context(&response_request);
    let configured = cors_route(app(), Method::GET, "/items/{id}", policy()).unwrap();
    let route = &configured.routes["/items/{id}"][0];
    let wire = Context::new(route, &headers("https://app.example.invalid"), true)
        .finish(transport(response_context, Status::FORBIDDEN, false, true));
    assert_eq!(
        wire.headers()[names::VARY],
        "Access-Control-Request-Headers, Access-Control-Request-Method, Origin"
    );
    assert!(!wire
        .headers()
        .contains_key(names::ACCESS_CONTROL_ALLOW_METHODS));
    for name in [
        "Vary",
        "Access-Control-Allow-Origin",
        "Access-Control-Allow-Credentials",
        "Access-Control-Max-Age",
        "Set-Cookie",
        "Cache-Control",
    ] {
        let response = empty(response_context, Status::OK).unwrap();
        assert!(append_header_text(response, name, "owned").is_err());
    }
}
