//! Source-stage public attachment oracles; explicit fixture values only.
use super::*;

fn config(bytes: i64) -> RateLimits {
    rate_limits(8, 4, 8192, bytes, 10_000).unwrap()
}
fn route_app() -> App<(), Error> {
    let app = route(
        app_default(()),
        Method::GET,
        "/",
        public_policy(),
        |request: Request, _, ()| async move { empty(response_context(&request), Status::OK) },
    )
    .unwrap();
    rate_route(app, Method::GET, "/", rate_policy(1, 60_000).unwrap()).unwrap()
}
#[test]
fn checked_public_rate_configuration_rejects_missing_duplicate_and_unsupported_subjects() {
    assert!(default_options().rate_limits.is_none());
    assert!(rate_limits(0, 1, 1, 1_000_000, 10_000).is_err());
    assert!(rate_limits(1, 0, 1, 1_000_000, 10_000).is_err());
    assert!(rate_policy(0, 60_000).is_err());
    assert!(rate_subject(rate_policy(1, 60_000).unwrap(), 1, 60_000, 0, 60_000).is_err());
    assert!(rate_route(
        route_app(),
        Method::GET,
        "/",
        rate_policy(1, 60_000).unwrap()
    )
    .is_err());
    assert!(rate_route(
        route_app(),
        Method::POST,
        "/",
        rate_policy(1, 60_000).unwrap()
    )
    .is_err());
    let app = route(
        app_default(()),
        Method::GET,
        "/",
        public_policy(),
        |request: Request, _, ()| async move { empty(response_context(&request), Status::OK) },
    )
    .unwrap();
    assert!(rate_route(
        app,
        Method::GET,
        "/",
        rate_subject(rate_policy(1, 60_000).unwrap(), 1, 60_000, 1, 60_000).unwrap()
    )
    .is_err());
}
#[test]
fn actual_route_vec_capacity_and_retained_boundary_are_charged() {
    let mut first = route_app();
    let first_plan = RuntimeRate::checked(&mut first, &config(4 * 1024 * 1024), 1, 1).unwrap();
    let first_bytes = first_plan.plan.as_ref().unwrap().retained_bytes();
    let first_capacity = first.routes.get("/").unwrap().capacity();
    let mut second = route_app();
    second.routes.get_mut("/").unwrap().reserve_exact(9);
    let second_capacity = second.routes.get("/").unwrap().capacity();
    let second_plan = RuntimeRate::checked(&mut second, &config(4 * 1024 * 1024), 1, 1).unwrap();
    assert_eq!(
        second_plan.plan.as_ref().unwrap().retained_bytes() - first_bytes,
        (second_capacity - first_capacity) * size_of::<Route<()>>()
    );
    let mut exact = route_app();
    assert!(RuntimeRate::checked(
        &mut exact,
        &config(i64::try_from(first_bytes).unwrap()),
        1,
        1
    )
    .is_ok());
    let mut short = route_app();
    assert!(RuntimeRate::checked(
        &mut short,
        &config(i64::try_from(first_bytes - 1).unwrap()),
        1,
        1
    )
    .is_err());
}

#[test]
fn all_registered_routes_require_explicit_policy_and_capacity_revalidation() {
    let mut missing = route(
        app_default(()),
        Method::GET,
        "/",
        public_policy(),
        |request: Request, _, ()| async move { empty(response_context(&request), Status::OK) },
    )
    .unwrap();
    assert!(RuntimeRate::checked(&mut missing, &config(4 * 1024 * 1024), 1, 1).is_err());
    assert!(fixed_metadata(usize::MAX, 1).is_err());
    assert!(fixed_metadata(1, usize::MAX).is_err());
    let fixed = fixed_metadata(1, 1).unwrap();
    let options = with_rate_limits(
        capacity(default_options(), 1, 1).unwrap(),
        rate_limits(1, 1, 1, i64::try_from(fixed).unwrap(), 10000).unwrap(),
    )
    .unwrap();
    assert!(capacity(options, 2, 1).is_err());
}

#[tokio::test]
async fn established_expired_lease_wins_over_an_exhausted_subject_bucket() {
    let mut app =
        route(
            app_default(()),
            Method::GET,
            "/",
            authenticated_policy(|_, _| async {
                VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(30))
            }),
            |request: Request, _, _: AuthScope| async move {
                empty(response_context(&request), Status::OK)
            },
        )
        .unwrap();
    app = rate_route(
        app,
        Method::GET,
        "/",
        rate_subject(rate_policy(64, 60000).unwrap(), 1, 60000, 1, 60000).unwrap(),
    )
    .unwrap();
    let rates = RuntimeRate::checked(&mut app, &config(4 * 1024 * 1024), 1, 1).unwrap();
    let rate = rates.request(0).unwrap();
    let request = bounded_response_request_fixture(256);
    let context = response_context(&request);
    let owner = LeaseOwner::new_http(Instant::now() + Duration::from_secs(10)).unwrap();
    let _scope = AuthScope::bind(
        VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(30)).unwrap(),
        Arc::clone(&owner.lease),
    )
    .unwrap();
    assert!(
        before_handler(
            context,
            Some(&rate),
            &owner.lease,
            PolicySource::Bearer,
            false,
        )
        .await
        .is_none(),
        "first subject admission failed"
    );
    let other = LeaseOwner::new_http(Instant::now() + Duration::from_secs(10)).unwrap();
    let _scope = AuthScope::bind(
        VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(30)).unwrap(),
        Arc::clone(&other.lease),
    )
    .unwrap();
    let lease = Arc::clone(&other.lease);
    drop(other);
    let refusal = before_handler(context, Some(&rate), &lease, PolicySource::Bearer, false)
        .await
        .expect("retired lease must fail");
    assert_eq!(refusal.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn retry_after_rounding_and_internal_rate_failures_have_closed_distinct_outcomes() {
    for (nanoseconds, seconds) in [
        (1, 1),
        (1_000_000_000, 1),
        (1_000_000_001, 2),
        (u64::MAX, 18_446_744_074),
    ] {
        assert!(matches!(
            decision(RateDecision::Limited { retry_ns: nanoseconds }),
            Err(FallbackStatus::Limited { retry_seconds }) if retry_seconds == seconds
        ));
    }
    for failure in [RateDecision::Limited { retry_ns: 0 }, RateDecision::Fault] {
        assert!(matches!(decision(failure), Err(FallbackStatus::Internal)));
    }
    for unavailable in [
        RateDecision::Capacity,
        RateDecision::Closing,
        RateDecision::ClockFault,
    ] {
        let reason = decision(unavailable)
            .err()
            .expect("unavailable limiter must refuse");
        assert!(matches!(reason, FallbackStatus::Capacity));
        let request = bounded_response_request_fixture(256);
        let mut context = response_context(&request).share();
        context.profile.origin = Some(HeaderValue::from_static("https://app.example.invalid"));
        let ledger = response_send::SendLedger::new(
            Arc::clone(&context.producer),
            Arc::clone(&context.fallback),
        );
        let wire = HttpResponse::rate_refusal(&context, reason, PolicySource::Bearer, false)
            .transfer_native(&ledger, false)
            .unwrap();
        assert_eq!(wire.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            wire.headers()[names::ACCESS_CONTROL_ALLOW_ORIGIN],
            "https://app.example.invalid"
        );
        assert_eq!(
            wire.headers()[names::CONTENT_SECURITY_POLICY],
            CONTENT_SECURITY_POLICY
        );
        assert_eq!(wire.headers()[names::X_CONTENT_TYPE_OPTIONS], "nosniff");
    }
}
