//! Native proof of the closed observer's acknowledgements, not CI remediation.
use super::*;
use crate::auth::browser_login_observer::{Event, Exit, Gate, Observer, Phase as ObservedPhase};

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

#[tokio::test]
async fn observed_consumed_missing_row_live_denial() {
    let f = Fixture::new().await;
    let p = f.open(2, 3, 144).await.unwrap();
    let bootstrap = owner();
    p.bootstrap(&bootstrap, None).await.unwrap();
    let first = owner();
    drop(
        p.claim_http(&first, browser_input(&p, &bootstrap))
            .await
            .unwrap(),
    );
    let consumed = first.was_consumed() && count(&p).await == 0;
    let request = owner();
    let original = request.request_until().unwrap();
    let observer = Observer::new(&request.lease.lease);
    let before = p.business_enqueues.load(Ordering::SeqCst);
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        observer.scope(p.claim_http(&request, browser_input(&p, &bootstrap))),
    )
    .await;
    let denied = matches!(result, Ok(Err(ref e)) if e.login_status() == crate::http_server::Status::FORBIDDEN);
    let record = observer.snapshot();
    let live = request.request_until().is_ok_and(|until| until == original);
    let unchanged = !request.was_consumed()
        && p.business_enqueues.load(Ordering::SeqCst) == before
        && count(&p).await == 0;
    close(&p, 2000).await.unwrap();
    f.stop().await;
    assert!(consumed && denied && live && unchanged);
    assert!(record.row_missing && record.rollback_ack && record.restore_commit_ack);
    assert_eq!(
        (record.phase, record.event, record.poll_gate, record.exit),
        (
            ObservedPhase::RestoreCommit,
            Event::Ready,
            Gate::Live,
            Exit::None
        )
    );
}

#[tokio::test]
async fn observed_consumed_missing_row_actual_restoration_wait_gate_expiry() {
    let f = Fixture::new().await;
    let mut p = f.open(2, 3, 144).await.unwrap();
    let bootstrap = owner();
    p.bootstrap(&bootstrap, None).await.unwrap();
    let first = owner();
    drop(
        p.claim_http(&first, browser_input(&p, &bootstrap))
            .await
            .unwrap(),
    );
    let consumed = first.was_consumed() && count(&p).await == 0;
    let wait = Arc::new(PostcommitMaintenanceWait::new());
    p.postcommit_wait = Some(Arc::clone(&wait));
    let request = owner();
    let original = request.request_until().unwrap();
    let before = p.business_enqueues.load(Ordering::SeqCst);
    let observer = Observer::new(&request.lease.lease);
    let mut operation =
        Box::pin(observer.scope(p.claim_http(&request, browser_input(&p, &bootstrap))));
    let arrived = tokio::time::timeout(Duration::from_secs(2), async {
        tokio::select! { _ = wait.arrived.notified() => true, _ = &mut operation => false }
    })
    .await
    .is_ok_and(|arrived| arrived);
    let waiting = observer.snapshot();
    let live_before = request.request_until().is_ok_and(|until| until == original);
    request.lease.lease.gate.lock().unwrap().expires_at = Instant::now();
    let result = tokio::time::timeout(Duration::from_secs(2), &mut operation).await;
    let expired = matches!(result, Ok(Err(ref e)) if e.login_status() == crate::http_server::Status::GATEWAY_TIMEOUT);
    let terminal = observer.snapshot();
    drop(operation);
    wait.release.notify_one();
    let unchanged = !request.was_consumed()
        && p.business_enqueues.load(Ordering::SeqCst) == before
        && count(&p).await == 0;
    close(&p, 2000).await.unwrap();
    f.stop().await;
    assert!(consumed && arrived && live_before && expired && unchanged);
    assert!(waiting.row_missing && waiting.rollback_ack && !waiting.restore_commit_ack);
    assert_eq!(
        (
            waiting.phase,
            waiting.event,
            waiting.poll_gate,
            waiting.pending_phase,
            waiting.pending_gate
        ),
        (
            ObservedPhase::RestoreMaintenance,
            Event::Pending,
            Gate::Live,
            ObservedPhase::RestoreMaintenance,
            Gate::Live
        )
    );
    assert_eq!(
        (
            terminal.phase,
            terminal.event,
            terminal.poll_gate,
            terminal.exit,
            terminal.exit_gate
        ),
        (
            ObservedPhase::RestoreMaintenance,
            Event::Pending,
            Gate::Expired,
            Exit::ActualGate,
            Gate::Expired
        )
    );
    assert!(terminal.row_missing && terminal.rollback_ack && !terminal.restore_commit_ack);
    assert!(
        terminal.saw_live_pending
            && terminal.live_pending_phase == ObservedPhase::RestoreMaintenance
            && terminal.gate_refused
    );
}
