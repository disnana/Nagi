// 既存の取得予算oracleを本体adapterで再実行する。None予算はtest-only。
use super::adapter::{AcquireBudget, Adapter, AdapterSeams};
use super::{Config, Failure, Finish, Gate, Kind, Outcome, Tx};
use crate::FromRow;
use futures_util::FutureExt;
use rusqlite::Row;
use std::{future::Future, sync::Arc, time::Duration};
use tokio::time::Instant;

const CLOSE: Duration = Duration::from_secs(2);
const WATCHDOG: Duration = Duration::from_secs(3);

// watchdog/panicは業務Errと分け、Gate解放とnative終了の後でassertする。
async fn watch<F: Future>(future: F) -> Result<F::Output, &'static str> {
    match tokio::time::timeout(
        WATCHDOG,
        std::panic::AssertUnwindSafe(future).catch_unwind(),
    )
    .await
    {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(_)) => Err("acquire observation panicked"),
        Err(_) => Err("acquire watchdog expired"),
    }
}

// held checkoutのsetup失敗も、全barrier解放とcloseを試してから失敗にする。
async fn setup<T>(
    adapter: &Adapter,
    result: Result<Result<T, Failure>, &'static str>,
    gates: &[&Gate],
) -> T {
    let error = match result {
        Ok(Ok(value)) => return value,
        Ok(Err(error)) => format!("{error:?}"),
        Err(error) => error.to_owned(),
    };
    let mut release_failed = false;
    for gate in gates {
        release_failed |=
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| gate.release())).is_err();
    }
    let closed = watch(adapter.close(CLOSE)).await;
    let stats = adapter.observer().snapshot();
    panic!(
        "acquire setup failed: {error}; gate_release_failed={release_failed}; close={closed:?}; pending={}, starting={}",
        stats.pending_workers, stats.starting
    );
}

// duration fixtureはFuture構築時でなく、最初のpollで一度だけ予算を作る。
async fn begin_after(adapter: &Adapter, duration: Duration) -> Result<Tx, Failure> {
    let budget = AcquireBudget::after_at(Instant::now(), duration)
        .expect("bounded private duration fixture");
    adapter.begin_with_budget(budget).await
}

#[derive(Debug, PartialEq)]
struct Number(i64);
impl FromRow for Number {
    fn columns() -> &'static [&'static str] {
        &["n"]
    }
    fn read(row: &Row<'_>, indices: &[usize]) -> rusqlite::Result<Self> {
        assert!(!super::management_active());
        Ok(Self(row.get(indices[0])?))
    }
}

// 予想外の成功Txも終端してから、timeoutのoracleを判定する。
async fn expected_error(
    result: Result<Result<Tx, Failure>, &'static str>,
) -> Result<Failure, String> {
    match result {
        Ok(Err(error)) => Ok(error),
        Ok(Ok(tx)) => {
            let settled = watch(tx.finish(Finish::Rollback)).await;
            Err(format!(
                "unexpected successful acquisition; rollback={settled:?}"
            ))
        }
        Err(error) => Err(error.to_owned()),
    }
}

async fn successful_tx(result: Result<Result<Tx, Failure>, &'static str>) -> Result<(), String> {
    let tx = result
        .map_err(str::to_owned)?
        .map_err(|error| format!("{error:?}"))?;
    let queried = watch(tx.query::<Number>("SELECT 1 AS n", vec![])).await;
    let settled = watch(tx.finish(Finish::Rollback)).await;
    match (queried, settled) {
        (Ok(Ok(Some(Number(1)))), Ok(Ok(()))) => Ok(()),
        (queried, settled) => Err(format!("query={queried:?}; rollback={settled:?}")),
    }
}

fn assert_timeout(result: Result<Failure, String>) {
    let error = result.expect("business AcquireTimeout, not panic/watchdog/success");
    assert_eq!(error.kind, Kind::AcquireTimeout);
    assert_eq!(error.outcome, Outcome::NotApplicable);
    assert!(!error.retired);
    assert!(error.cleanup.is_none());
}

fn assert_joined(adapter: &Adapter, count: usize) {
    let stats = adapter.observer().snapshot();
    assert_eq!(stats.created, count);
    assert_eq!(stats.native_started, count);
    assert_eq!(stats.native_closed, count);
    assert_eq!(stats.joined, count);
    assert_eq!(stats.start_failed, 0);
    assert_eq!(stats.starting, 0);
    assert_eq!(stats.pending_workers, 0);
}

#[test]
fn acquire_budget_arithmetic_keeps_immediate_distinct_and_checks_overflow() {
    let now = Instant::now();
    let duration = Duration::from_secs(8);
    assert_eq!(
        AcquireBudget::after_at(now, Duration::ZERO),
        Some(AcquireBudget::Immediate)
    );
    let budget = AcquireBudget::after_at(now, duration).unwrap();
    assert_eq!(budget, AcquireBudget::Deadline(now + duration));
    assert_eq!(budget.remaining_at(now), duration);
    assert_eq!(
        budget.remaining_at(now + Duration::from_secs(3)),
        Duration::from_secs(5)
    );
    assert_eq!(budget.remaining_at(now + duration), Duration::ZERO);
    assert_eq!(
        budget.remaining_at(now + duration + duration),
        Duration::ZERO
    );
    assert_eq!(AcquireBudget::Immediate.remaining_at(now), Duration::ZERO);
    assert_eq!(AcquireBudget::after_at(now, Duration::MAX), None);
}

#[tokio::test]
async fn acquire_immediate_free_reserves_before_slow_startup_and_reuses() {
    let startup = Arc::new(Gate::new());
    let adapter = Adapter::new(
        Config::default(),
        AdapterSeams {
            startup: Some(Arc::clone(&startup)),
            ..AdapterSeams::default()
        },
    );
    let mut get = Box::pin(begin_after(&adapter, Duration::ZERO));
    let first_poll_pending = futures_util::poll!(&mut get).is_pending();
    let entered = watch(startup.wait()).await;
    let reserved = adapter.observer().snapshot();
    startup.release();
    let first = successful_tx(watch(&mut get).await).await;
    drop(get);
    let returned = watch(adapter.observer().wait_returned(1)).await;
    let second = successful_tx(watch(begin_after(&adapter, Duration::ZERO)).await).await;
    let closed = watch(adapter.close(CLOSE)).await;
    assert!(first_poll_pending);
    assert!(entered.is_ok());
    assert_eq!(reserved.created, 1);
    assert_eq!(reserved.native_started, 1);
    assert!(returned.is_ok());
    first.unwrap();
    second.unwrap();
    closed.unwrap().unwrap();
    assert_joined(&adapter, 1);
}

#[tokio::test]
async fn acquire_immediate_logical_full_is_timeout_without_pool_failure() {
    let adapter = Adapter::new(Config::default(), AdapterSeams::default());
    let held = setup(&adapter, watch(adapter.checkout_without_begin()).await, &[]).await;
    let failed =
        expected_error(watch(adapter.begin_with_budget(AcquireBudget::Immediate)).await).await;
    let unchanged = adapter.observer().snapshot();
    drop(held);
    let reused =
        successful_tx(watch(adapter.begin_with_budget(AcquireBudget::Immediate)).await).await;
    let closed = watch(adapter.close(CLOSE)).await;
    assert_timeout(failed);
    assert_eq!(unchanged.created, 1);
    assert_eq!(unchanged.native_started, 1);
    assert_eq!(unchanged.admitted, 0);
    assert!(!unchanged.closing);
    reused.unwrap();
    closed.unwrap().unwrap();
    assert_joined(&adapter, 1);
}

#[tokio::test]
async fn acquire_immediate_native_full_after_checkout_detach_is_timeout() {
    let detaching = Arc::new(Gate::new());
    let publication = Arc::new(Gate::new());
    let adapter = Adapter::new(
        Config::default(),
        AdapterSeams {
            target_worker: Some(1),
            detaching: Some(Arc::clone(&detaching)),
            publication: Some(Arc::clone(&publication)),
            ..AdapterSeams::default()
        },
    );
    let held = setup(
        &adapter,
        watch(adapter.checkout_without_begin()).await,
        &[&detaching, &publication],
    )
    .await;
    let taking = std::thread::spawn(move || held.take_and_drop());
    let entered = watch(detaching.wait()).await;
    let failed =
        expected_error(watch(adapter.begin_with_budget(AcquireBudget::Immediate)).await).await;
    // finiteは期限前にnative fenceへ到達して待つ。入口のexpired検査だけでは足りない。
    let mut finite = Box::pin(adapter.begin_with_budget(AcquireBudget::Deadline(
        Instant::now() + Duration::from_secs(1),
    )));
    let first_finite_poll = futures_util::poll!(&mut finite);
    let finite_pending = first_finite_poll.is_pending();
    let finite_result = match first_finite_poll {
        std::task::Poll::Ready(result) => Ok(result),
        std::task::Poll::Pending => watch(&mut finite).await,
    };
    let finite_failed = expected_error(finite_result).await;
    drop(finite);
    let unchanged = adapter.observer().snapshot();
    detaching.release();
    let taken = taking.join();
    let published = watch(publication.wait()).await;
    publication.release();
    let joined = watch(adapter.observer().wait_joined(1)).await;
    let replacement = successful_tx(watch(adapter.begin()).await).await;
    let closed = watch(adapter.close(CLOSE)).await;
    assert!(entered.is_ok());
    assert_timeout(failed);
    assert!(
        finite_pending,
        "finite budget should wait at the occupied native fence"
    );
    assert_timeout(finite_failed);
    assert_eq!(unchanged.created, 1);
    assert_eq!(unchanged.native_started, 1);
    assert_eq!(unchanged.pending_workers, 1);
    assert!(!unchanged.closing);
    assert!(taken.is_ok() && published.is_ok() && joined.is_ok());
    replacement.unwrap();
    closed.unwrap().unwrap();
    assert_joined(&adapter, 2);
}

#[tokio::test]
async fn acquire_absolute_budget_survives_logical_wait_to_native_fence() {
    let detaching = Arc::new(Gate::new());
    let publication = Arc::new(Gate::new());
    let adapter = Adapter::new(
        Config::default(),
        AdapterSeams {
            target_worker: Some(1),
            detaching: Some(Arc::clone(&detaching)),
            publication: Some(Arc::clone(&publication)),
            ..AdapterSeams::default()
        },
    );
    let held = setup(
        &adapter,
        watch(adapter.checkout_without_begin()).await,
        &[&detaching, &publication],
    )
    .await;
    let deadline = Instant::now() + Duration::from_millis(200);
    let mut get = Box::pin(adapter.begin_with_budget(AcquireBudget::Deadline(deadline)));
    let logical_pending = futures_util::poll!(&mut get).is_pending();
    let taking = std::thread::spawn(move || held.take_and_drop());
    let entered = watch(detaching.wait()).await;
    // Gateがpermit返却と旧nativeの生存を確定する。sleepは期限到達だけに使う。
    tokio::time::sleep_until(deadline).await;
    let after_deadline = futures_util::poll!(&mut get);
    let original_budget_expired = matches!(
        &after_deadline,
        std::task::Poll::Ready(Err(error)) if error.kind == Kind::AcquireTimeout
    );
    let result = match after_deadline {
        std::task::Poll::Ready(result) => Ok(result),
        std::task::Poll::Pending => watch(&mut get).await,
    };
    let failed = expected_error(result).await;
    drop(get);
    let unchanged = adapter.observer().snapshot();
    detaching.release();
    let taken = taking.join();
    let published = watch(publication.wait()).await;
    publication.release();
    let joined = watch(adapter.observer().wait_joined(1)).await;
    let replacement = successful_tx(watch(adapter.begin()).await).await;
    let closed = watch(adapter.close(CLOSE)).await;
    assert!(logical_pending && entered.is_ok());
    assert!(
        original_budget_expired,
        "original absolute budget must already be expired at native fence"
    );
    assert_timeout(failed);
    assert_eq!(unchanged.created, 1);
    assert_eq!(unchanged.native_started, 1);
    assert_eq!(unchanged.pending_workers, 1);
    assert!(!unchanged.closing);
    assert!(taken.is_ok() && published.is_ok() && joined.is_ok());
    replacement.unwrap();
    closed.unwrap().unwrap();
    assert_joined(&adapter, 2);
}

#[tokio::test]
async fn acquire_expired_finite_budget_does_not_register_even_when_native_free() {
    let adapter = Adapter::new(Config::default(), AdapterSeams::default());
    let expired = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
    let failed =
        expected_error(watch(adapter.begin_with_budget(AcquireBudget::Deadline(expired))).await)
            .await;
    let unchanged = adapter.observer().snapshot();
    let next =
        successful_tx(watch(adapter.begin_with_budget(AcquireBudget::Immediate)).await).await;
    let closed = watch(adapter.close(CLOSE)).await;
    assert_timeout(failed);
    assert_eq!(unchanged.created, 0);
    assert_eq!(unchanged.native_started, 0);
    assert_eq!(unchanged.starting, 0);
    assert_eq!(unchanged.pending_workers, 0);
    assert!(!unchanged.closing);
    next.unwrap();
    closed.unwrap().unwrap();
    assert_joined(&adapter, 1);
}

#[tokio::test]
async fn acquire_same_task_explicit_budgets_are_isolated_and_cancel_preserves_legacy_begin() {
    let detaching = Arc::new(Gate::new());
    let publication = Arc::new(Gate::new());
    let adapter = Adapter::new(
        Config::default(),
        AdapterSeams {
            target_worker: Some(1),
            detaching: Some(Arc::clone(&detaching)),
            publication: Some(Arc::clone(&publication)),
            ..AdapterSeams::default()
        },
    );
    let held = setup(
        &adapter,
        watch(adapter.checkout_without_begin()).await,
        &[&detaching, &publication],
    )
    .await;
    let taking = std::thread::spawn(move || held.take_and_drop());
    let entered = watch(detaching.wait()).await;
    // semaphore permitは返却済み。longはManagerが予算を読むnative fenceまで進む。
    let mut long = Box::pin(adapter.begin_with_budget(AcquireBudget::Deadline(
        Instant::now() + Duration::from_secs(20),
    )));
    let long_pending = futures_util::poll!(&mut long).is_pending();
    let expired =
        AcquireBudget::Deadline(Instant::now().checked_sub(Duration::from_secs(1)).unwrap());
    let short = expected_error(watch(adapter.begin_with_budget(expired)).await).await;
    let long_still_pending = futures_util::poll!(&mut long).is_pending();
    drop(long);
    let unchanged = adapter.observer().snapshot();
    detaching.release();
    let taken = taking.join();
    let published = watch(publication.wait()).await;
    publication.release();
    let joined = watch(adapter.observer().wait_joined(1)).await;
    // 新native登録を必要にする。健康worker再貸出だけではscope漏れを隠し得る。
    let legacy = successful_tx(watch(adapter.begin()).await).await;
    let closed = watch(adapter.close(CLOSE)).await;
    assert!(entered.is_ok() && long_pending && long_still_pending);
    assert_timeout(short);
    assert_eq!(unchanged.created, 1);
    assert_eq!(unchanged.native_started, 1);
    assert_eq!(unchanged.pending_workers, 1);
    assert!(!unchanged.closing);
    assert!(taken.is_ok() && published.is_ok() && joined.is_ok());
    legacy.unwrap();
    closed.unwrap().unwrap();
    assert_joined(&adapter, 2);
}

#[tokio::test]
async fn acquire_registered_startup_and_begin_are_outside_finite_budget() {
    let startup = Arc::new(Gate::new());
    let begin = Arc::new(Gate::new());
    let adapter = Adapter::new(
        Config {
            begin_gate: Some(Arc::clone(&begin)),
            ..Config::default()
        },
        AdapterSeams {
            startup: Some(Arc::clone(&startup)),
            ..AdapterSeams::default()
        },
    );
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut get = Box::pin(adapter.begin_with_budget(AcquireBudget::Deadline(deadline)));
    let first_pending = futures_util::poll!(&mut get).is_pending();
    let startup_entered = watch(startup.wait()).await;
    let reserved = adapter.observer().snapshot();
    tokio::time::sleep_until(deadline).await;
    let after_deadline = futures_util::poll!(&mut get);
    let startup_still_pending = after_deadline.is_pending();
    startup.release();
    // poll getを継続しながら、deadlineを過ぎたnative BEGINもGateで観測する。
    let begin_entered = if startup_still_pending {
        tokio::select! {
            entered = watch(begin.wait()) => entered,
            result = &mut get => {
                let _ = expected_error(Ok(result)).await;
                Err("begin replied before gated BEGIN")
            }
        }
    } else {
        if let std::task::Poll::Ready(result) = after_deadline {
            let _ = expected_error(Ok(result)).await;
        }
        Err("startup was timed out after registration")
    };
    begin.release();
    let acquired = if begin_entered.is_ok() {
        successful_tx(watch(&mut get).await).await
    } else {
        drop(get);
        Err("registered acquisition did not reach BEGIN".to_owned())
    };
    let closed = watch(adapter.close(CLOSE)).await;
    assert!(first_pending && startup_entered.is_ok() && startup_still_pending);
    assert_eq!(reserved.created, 1);
    assert_eq!(reserved.native_started, 1);
    assert!(begin_entered.is_ok());
    acquired.unwrap();
    closed.unwrap().unwrap();
    assert_joined(&adapter, 1);
}

#[tokio::test]
async fn acquire_expiry_does_not_replace_closed_or_recycle_failure_cause() {
    let adapter = Adapter::new(Config::default(), AdapterSeams::default());
    let initially_closed = watch(adapter.close(CLOSE)).await;
    let expired =
        AcquireBudget::Deadline(Instant::now().checked_sub(Duration::from_secs(1)).unwrap());
    let closed_error = expected_error(watch(adapter.begin_with_budget(expired)).await).await;
    initially_closed.unwrap().unwrap();
    assert_eq!(closed_error.unwrap().kind, Kind::Closed);
    assert_joined(&adapter, 0);

    let adapter = Adapter::new(
        Config::default(),
        AdapterSeams {
            fail_recycle: true,
            ..AdapterSeams::default()
        },
    );
    let first = successful_tx(watch(adapter.begin()).await).await;
    let returned = watch(adapter.observer().wait_returned(1)).await;
    let recycle_error =
        expected_error(watch(adapter.begin_with_budget(AcquireBudget::Immediate)).await).await;
    let failure_error = expected_error(watch(adapter.begin_with_budget(expired)).await).await;
    let closed = watch(adapter.close(CLOSE)).await;
    first.unwrap();
    assert!(returned.is_ok());
    let error = recycle_error.unwrap();
    assert_eq!(error.kind, Kind::Worker);
    assert!(error.retired && error.cleanup.is_some());
    let repeated = failure_error.unwrap();
    assert_eq!(repeated.kind, Kind::Worker);
    assert_eq!(
        repeated
            .cleanup
            .as_ref()
            .map(|e| (&e.message, crate::error_kind(e))),
        error
            .cleanup
            .as_ref()
            .map(|e| (&e.message, crate::error_kind(e)))
    );
    assert!(repeated.retired);
    assert!(closed.unwrap().is_err());
    assert_joined(&adapter, 1);
}
