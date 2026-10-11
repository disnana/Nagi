//! Bounded actual-queue expiry oracles, including BrowserLogin phase boundaries.
use super::*;

fn browser_input(pool: &Pool, bootstrap: &RequestOwner) -> browser::LoginInput {
    let cell = bootstrap.cell.lock().unwrap();
    let pair = cell.pair.as_ref().unwrap();
    let mut headers = hyper::header::HeaderMap::new();
    headers.insert(
        hyper::header::AUTHORIZATION,
        hyper::header::HeaderValue::from_static("Bearer owned"),
    );
    headers.insert(
        hyper::header::ORIGIN,
        hyper::header::HeaderValue::from_static("https://example.invalid"),
    );
    headers.insert(
        hyper::header::COOKIE,
        hyper::header::HeaderValue::from_str(&format!("__Host-prelogin={}", pair.handle)).unwrap(),
    );
    headers.insert(
        "x-csrf-token",
        hyper::header::HeaderValue::from_str(&pair.token).unwrap(),
    );
    pool.login_input(&headers).unwrap()
}

async fn stored_expiry(pool: &Pool) -> i64 {
    let tx = sqlite::begin(&pool.native, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let expiry = sqlite::query::<Count>(
        &tx,
        sqlite::literal("SELECT expiry_ms AS n FROM __nagi_prelogin_ready"),
        sqlite::parameters(),
    )
    .await
    .unwrap()
    .unwrap()
    .0;
    sqlite::rollback(tx).await.unwrap();
    expiry
}

async fn claim_then_verify(
    pool: &Pool,
    request: &RequestOwner,
    input: browser::LoginInput,
    calls: &AtomicUsize,
) -> Result<VerifiedIdentity, Failure> {
    pool.claim_http(request, input)
        .await?
        .verify(|| async {
            calls.fetch_add(1, Ordering::SeqCst);
            VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(60))
        })
        .await
}

#[tokio::test]
async fn http_claim_already_expired_row_returns_403_before_gate_shortening() {
    let f = Fixture::new().await;
    let mut p = f.open(2, 3, 144).await.unwrap();
    let bootstrap = owner();
    p.bootstrap(&bootstrap, None).await.unwrap();
    let input = browser_input(&p, &bootstrap);
    let expiry = stored_expiry(&p).await;
    // The stored row exists, but equality at its initial validity sample is
    // expired. The original actual request gate remains live and unchanged.
    p.context.set_test_clock_sequence(vec![expiry; 8]);
    let request = owner();
    let original_until = request.request_until().unwrap();
    let enqueues = p.business_enqueues.load(Ordering::SeqCst);
    let calls = AtomicUsize::new(0);
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        claim_then_verify(&p, &request, input, &calls),
    )
    .await
    .expect("finite already-expired validity watchdog");
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("initially expired row returned authority"),
    };
    assert!(error.kind == FailureKind::Denied && matches!(error.origin, FailureOrigin::Core));
    assert!(error.login_status() == crate::http_server::Status::FORBIDDEN);
    assert!(error.outcome == sqlite::Outcome::RolledBack && !error.business_committed);
    assert!(request.request_until().unwrap() == original_until);
    assert!(!request.was_consumed() && calls.load(Ordering::SeqCst) == 0);
    assert!(p.business_enqueues.load(Ordering::SeqCst) == enqueues);
    close(&p, 2000).await.unwrap();
    f.stop().await;
}

#[tokio::test]
async fn http_claim_live_row_gate_expires_504_during_real_preconsume_reserve_wait() {
    let f = Fixture::new().await;
    let mut p = f.open(2, 3, 144).await.unwrap();
    let bootstrap = owner();
    p.bootstrap(&bootstrap, None).await.unwrap();
    let input = browser_input(&p, &bootstrap);
    let expiry = stored_expiry(&p).await;
    // UTC stays 400ms before the row expiry. The actual monotonic gate, rather
    // than a later row-only UTC check, must end this pending claim.
    p.context.set_test_clock_sequence(vec![expiry - 400; 8]);
    let wait = Arc::new(PostcommitMaintenanceWait::new());
    p.preconsume_wait = Some(Arc::clone(&wait));
    let request = owner();
    let original_until = request.request_until().unwrap();
    let enqueues = p.business_enqueues.load(Ordering::SeqCst);
    let calls = AtomicUsize::new(0);
    let mut operation = Box::pin(claim_then_verify(&p, &request, input, &calls));
    tokio::time::timeout(Duration::from_secs(2), async {
        tokio::select! {
            _ = wait.arrived.notified() => {},
            _ = &mut operation => panic!("claim finished before actual reserve wait"),
        }
    })
    .await
    .expect("finite preconsume arrival watchdog");
    let tightened_until = request.request_until().unwrap();
    assert!(tightened_until < original_until && Instant::now() < tightened_until);
    assert!(tightened_until <= Instant::now() + Duration::from_millis(400));
    assert!(!request.was_consumed() && calls.load(Ordering::SeqCst) == 0);
    assert!(p.business_enqueues.load(Ordering::SeqCst) == enqueues);
    // No release or injected wake: the reset Sleep ends the actual claim.
    let result = tokio::time::timeout(Duration::from_millis(800), &mut operation)
        .await
        .expect("finite tightened-gate wake watchdog");
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("expired shortened gate returned authority"),
    };
    assert!(error.kind == FailureKind::Expired && matches!(error.origin, FailureOrigin::Authority));
    assert!(error.login_status() == crate::http_server::Status::GATEWAY_TIMEOUT);
    assert!(Instant::now() >= tightened_until && Instant::now() < original_until);
    // 504 conveys no native commit/rollback outcome. Here the bounded queue
    // seam independently proves that no business DELETE was even admitted.
    assert!(error.outcome == sqlite::Outcome::NotApplicable && !error.business_committed);
    assert!(!request.was_consumed() && calls.load(Ordering::SeqCst) == 0);
    assert!(p.business_enqueues.load(Ordering::SeqCst) == enqueues);
    drop(operation);
    // Read actual DB state after dropping the claim and its queue permits;
    // preservation is observed here, not inferred from the timeout status.
    assert!(count(&p).await == 1);
    close(&p, 2000).await.unwrap();
    f.stop().await;
}

#[tokio::test]
async fn bootstrap_ttl_elapsed_during_actual_maintenance_wait_never_becomes_ready() {
    let f = Fixture::new().await;
    let result = open(
        &f.store,
        sqlite::options(1, 2, 1000, 1000).unwrap(),
        options(2, 3, 144, 1, 20, 2, 8, 10000, 4, 16).unwrap(),
        cookie_options("__Host-prelogin", session::SameSite::Strict).unwrap(),
        "https://example.invalid",
    )
    .await;
    if !cfg!(any(unix, windows)) {
        assert!(result.is_err());
        f.stop().await;
        return;
    }
    let mut p = result.unwrap();
    let wait = Arc::new(PostcommitMaintenanceWait::new());
    p.postcommit_wait = Some(Arc::clone(&wait));
    // Hold UTC steady so elapsed monotonic time, not a later UTC reading, is
    // the refusal reason. This is a private <=16 clock sequence, not authority.
    let tx = sqlite::begin(&p.native, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let b = super::super::metadata(&tx).await.unwrap().last_wall;
    sqlite::rollback(tx).await.unwrap();
    p.context.set_test_clock_sequence(vec![b + 1; 7]);
    let request = owner(); // original request lease10s remains independent.
    let mut operation = Box::pin(p.bootstrap(&request, None));
    tokio::time::timeout(Duration::from_secs(2), async {
        tokio::select! {
            _ = wait.arrived.notified() => {},
            _ = &mut operation => panic!("bootstrap completed before maintenance wait"),
        }
    })
    .await
    .expect("finite maintenance arrival watchdog");
    // Arrival follows the actual initial sample and both actual business and
    // maintenance Tx acquisitions. Both real queue permits remain held and
    // the real maintenance future was observed Pending. Wait only for TTL,
    // never use sleep as proof of queue arrival or full native finish.
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(request.lease.lease.validate().is_ok());
    assert!(!request.is_ready());
    wait.release.notify_one();
    let result = tokio::time::timeout(Duration::from_secs(2), operation)
        .await
        .expect("finite maintenance finish watchdog");
    let e = result.expect_err("elapsed TTL must refuse readiness");
    assert!(e.kind == FailureKind::Unavailable);
    assert!(e.business_committed && e.outcome == sqlite::Outcome::Committed);
    assert!(p.business_enqueues.load(Ordering::SeqCst) == 1);
    assert!(count(&p).await == 1); // bodycommit stays counted; UTC cleanup not expired.
    assert!(!request.is_ready() && request.cell.lock().unwrap().pair.is_none());
    assert!(request.lease.lease.validate().is_ok());
    close(&p, 2000).await.unwrap();
    f.stop().await;
}

#[test]
fn bootstrap_projection_is_conservative_and_unknown_remaining_time_refuses() {
    let now = Instant::now();
    let clock = session::PreloginClock {
        monotonic: now,
        floor_ms: 1000,
        ceil_ms: 1001,
    };
    assert!(bootstrap_deadline(&clock, 1100).unwrap() == now + Duration::from_millis(99));
    // Equality is expired; negative/overflow remaining time is never full TTL.
    assert!(bootstrap_deadline(&clock, 1001).is_err());
    assert!(bootstrap_deadline(&clock, 1000).is_err());
    let overflow = session::PreloginClock {
        monotonic: now,
        floor_ms: 0,
        ceil_ms: i64::MAX,
    };
    assert!(bootstrap_deadline(&overflow, i64::MIN).is_err());
}

#[tokio::test]
async fn http_claim_tightened_gate_wakes_during_real_postcommit_maintenance_wait() {
    let f = Fixture::new().await;
    let mut p = f.open(2, 3, 144).await.unwrap();
    let boot = owner();
    p.bootstrap(&boot, None).await.unwrap();
    let input = {
        let cell = boot.cell.lock().unwrap();
        let pair = cell.pair.as_ref().unwrap();
        let mut headers = hyper::header::HeaderMap::new();
        headers.insert(
            hyper::header::AUTHORIZATION,
            hyper::header::HeaderValue::from_static("Bearer owned"),
        );
        headers.insert(
            hyper::header::ORIGIN,
            hyper::header::HeaderValue::from_static("https://example.invalid"),
        );
        headers.insert(
            hyper::header::COOKIE,
            hyper::header::HeaderValue::from_str(&format!("__Host-prelogin={}", pair.handle))
                .unwrap(),
        );
        headers.insert(
            "x-csrf-token",
            hyper::header::HeaderValue::from_str(&pair.token).unwrap(),
        );
        p.login_input(&headers).unwrap()
    };
    let tx = sqlite::begin(&p.native, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let b = super::super::metadata(&tx).await.unwrap().last_wall;
    sqlite::rollback(tx).await.unwrap();
    // A finite trusted UTC sequence leaves about400ms of the existing row's
    // TTL, while the actual request starts with10s. UTC then stays steady.
    p.context.set_test_clock_sequence(vec![b + 600; 8]);
    let wait = Arc::new(PostcommitMaintenanceWait::new());
    p.postcommit_wait = Some(Arc::clone(&wait));
    let request = owner();
    let mut operation = Box::pin(p.claim_http(&request, input));
    tokio::time::timeout(Duration::from_secs(2), async {
        tokio::select! {
            _ = wait.arrived.notified() => {},
            _ = &mut operation => panic!("claim completed before actual maintenance wait"),
        }
    })
    .await
    .unwrap();
    assert!(request.was_consumed());
    // No release/wake is injected: the reset Sleep itself must wake this
    // pending operation at the newly shortened actual gate, before10s.
    let result = tokio::time::timeout(Duration::from_millis(800), &mut operation)
        .await
        .unwrap();
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("expired claim returned authority"),
    };
    assert!(error.kind == FailureKind::Expired && error.business_committed);
    assert!(error.login_status() == crate::http_server::Status::GATEWAY_TIMEOUT);
    drop(operation);
    assert!(count(&p).await == 0 && request.was_consumed());
    close(&p, 2000).await.unwrap();
    f.stop().await;
}
