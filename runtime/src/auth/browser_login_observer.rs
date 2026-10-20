//! Opt-in diagnostic only. Fixed request-local state; no wire, credentials,
//! SQL, database values, paths, error strings, timestamps or duration guesses.
//! Pending means an actual future poll, not a conclusion inferred from 504.
use super::Lease;
use std::{
    fmt,
    future::Future,
    sync::{Arc, Mutex, Weak},
    time::Instant,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Phase {
    Entry,
    Body,
    ClaimBegin,
    ClaimCheck,
    ClaimRow,
    BusinessReserve,
    BusinessDelete,
    BusinessMaintenance,
    BusinessCommit,
    BusinessRollback,
    RestoreAcquire,
    RestoreCheck,
    RestoreMaintenance,
    RestoreCommit,
    RestoreRollback,
    Prepare,
    Handler,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Exit {
    None,
    BodyTimer,
    BodyLateReady,
    ClaimTimer,
    ClaimLateReady,
    ActualGate,
    PrepareTimer,
    PrepareLateReady,
    HandlerTimer,
    HandlerLateReady,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Gate {
    Unknown,
    Live,
    Expired,
    Inactive,
    Poisoned,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Event {
    Unpolled,
    Pending,
    Ready,
}
#[derive(Clone, Copy)]
pub(crate) struct Record {
    pub(crate) request: u64,
    pub(crate) phase: Phase,
    pub(crate) event: Event,
    pub(crate) poll_gate: Gate,
    pub(crate) before_gate: Gate,
    pub(crate) pending_phase: Phase,
    pub(crate) pending_gate: Gate,
    pub(crate) saw_pending: bool,
    pub(crate) saw_live_pending: bool,
    pub(crate) live_pending_phase: Phase,
    pub(crate) gate_refused: bool,
    pub(crate) row_missing: bool,
    pub(crate) rollback_ack: bool,
    pub(crate) restore_commit_ack: bool,
    pub(crate) exit: Exit,
    pub(crate) exit_gate: Gate,
}
#[derive(Clone)]
pub(crate) struct Observer(Arc<State>);
struct State {
    lease: Weak<Lease>,
    record: Mutex<Record>,
}
tokio::task_local! { static ACTIVE: Observer; }
impl Observer {
    pub(crate) fn new(lease: &Arc<Lease>) -> Self {
        Self(Arc::new(State {
            lease: Arc::downgrade(lease),
            record: Mutex::new(Record {
                request: lease.id,
                phase: Phase::Entry,
                event: Event::Unpolled,
                poll_gate: Gate::Unknown,
                before_gate: Gate::Unknown,
                pending_phase: Phase::Entry,
                pending_gate: Gate::Unknown,
                saw_pending: false,
                saw_live_pending: false,
                live_pending_phase: Phase::Entry,
                gate_refused: false,
                row_missing: false,
                rollback_ack: false,
                restore_commit_ack: false,
                exit: Exit::None,
                exit_gate: Gate::Unknown,
            }),
        }))
    }
    pub(crate) async fn scope<F: Future>(&self, future: F) -> F::Output {
        ACTIVE.scope(self.clone(), future).await
    }
    fn gate(&self) -> Gate {
        let Some(lease) = self.0.lease.upgrade() else {
            return Gate::Inactive;
        };
        let Ok(gate) = lease.gate.lock() else {
            return Gate::Poisoned;
        };
        if !gate.active {
            Gate::Inactive
        } else if gate.expires_at <= Instant::now() {
            Gate::Expired
        } else {
            Gate::Live
        }
    }
    fn update(&self, action: impl FnOnce(&mut Record, Gate)) {
        // Never retain a lifecycle gate while recording or formatting.
        let gate = self.gate();
        if let Ok(mut record) = self.0.record.lock() {
            action(&mut record, gate);
        }
    }
    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> Record {
        *self.0.record.lock().unwrap()
    }
}
pub(crate) async fn observe<F: Future>(phase: Phase, future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    std::future::poll_fn(|cx| {
        let before = ACTIVE
            .try_with(|observer| observer.gate())
            .unwrap_or(Gate::Unknown);
        let result = future.as_mut().poll(cx);
        let _ = ACTIVE.try_with(|observer| {
            observer.update(|record, gate| {
                record.phase = phase;
                record.poll_gate = gate;
                record.before_gate = before;
                record.event = if result.is_pending() {
                    Event::Pending
                } else {
                    Event::Ready
                };
                if result.is_pending() {
                    record.saw_pending = true;
                    record.pending_phase = phase;
                    record.pending_gate = gate;
                    if gate == Gate::Live {
                        record.saw_live_pending = true;
                        record.live_pending_phase = phase;
                    }
                }
            })
        });
        result
    })
    .await
}
pub(crate) fn row_missing() {
    let _ = ACTIVE.try_with(|observer| observer.update(|r, _| r.row_missing = true));
}
pub(crate) fn rollback_ack() {
    let _ = ACTIVE.try_with(|observer| observer.update(|r, _| r.rollback_ack = true));
}
pub(crate) fn restore_commit_ack() {
    let _ = ACTIVE.try_with(|observer| observer.update(|r, _| r.restore_commit_ack = true));
}
pub(crate) fn exit(exit: Exit) {
    let _ = ACTIVE.try_with(|observer| {
        observer.update(|r, gate| {
            r.exit = exit;
            r.gate_refused |= exit == Exit::ActualGate;
            r.exit_gate = gate;
        })
    });
}
// Only 504 failures emit; a stock build contains neither this state nor output.
fn failure_record(status: u16, record: Record, consumed: bool) -> Option<Closed> {
    (status == 504).then_some(Closed { record, consumed })
}
pub(crate) fn emit(status: u16, consumed: bool) {
    if status != 504 {
        return;
    }
    let _ = ACTIVE.try_with(|observer| {
        let record = observer.0.record.lock().ok().map(|r| *r);
        if let Some(record) = record.and_then(|r| failure_record(status, r, consumed)) {
            eprintln!("{record}");
        }
    });
}
struct Closed {
    record: Record,
    consumed: bool,
}
impl fmt::Display for Closed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.record;
        write!(f, "browser_login_phase_diag request={} status=504 phase={} event={} before_gate={} poll_gate={} saw_pending={} pending_phase={} pending_gate={} saw_live_pending={} live_pending_phase={} gate_refused={} row_missing={} rollback_ack={} restore_commit_ack={} exit={} exit_gate={} consumed={}",
            r.request, r.phase as u8, r.event as u8, r.before_gate as u8, r.poll_gate as u8,
            u8::from(r.saw_pending), r.pending_phase as u8, r.pending_gate as u8,
            u8::from(r.saw_live_pending), r.live_pending_phase as u8, u8::from(r.gate_refused),
            u8::from(r.row_missing), u8::from(r.rollback_ack), u8::from(r.restore_commit_ack),
            r.exit as u8, r.exit_gate as u8, u8::from(self.consumed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::LeaseOwner;
    use std::{task::Poll, time::Duration};
    fn owner() -> LeaseOwner {
        LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap()
    }
    #[tokio::test]
    async fn closed_output_excludes_future_payload_and_nonfailure_output() {
        let owner = owner();
        let observer = Observer::new(&owner.lease);
        let secret = "credential=/private/path SQL SELECT error-detail";
        let output = observer
            .scope(observe(Phase::ClaimRow, async { secret }))
            .await;
        assert_eq!(output, secret);
        let record = observer.snapshot();
        let text = Closed {
            record,
            consumed: false,
        }
        .to_string();
        assert!(!text.contains(secret) && text.len() < 512);
        assert!(text
            .strip_prefix("browser_login_phase_diag ")
            .unwrap()
            .split(' ')
            .all(|field| {
                let (_, value) = field.split_once('=').unwrap();
                !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
            }));
        assert!(failure_record(403, record, false).is_none());
        assert!(failure_record(200, record, true).is_none());
        assert!(failure_record(504, record, false).is_some());
        observer
            .scope(async {
                emit(403, false);
                emit(200, true);
            })
            .await;
        assert_eq!(observer.snapshot().exit, Exit::None);
    }
    #[tokio::test]
    async fn actual_pending_live_and_late_ready_are_distinct_and_request_local() {
        let owner = owner();
        let observer = Observer::new(&owner.lease);
        let other_owner = super::tests::owner();
        let other = Observer::new(&other_owner.lease);
        let mut pending = std::pin::pin!(observer.scope(observe(
            Phase::RestoreMaintenance,
            std::future::pending::<()>()
        )));
        assert!(
            std::future::poll_fn(|cx| Poll::Ready(pending.as_mut().poll(cx).is_pending())).await
        );
        let live = observer.snapshot();
        assert_eq!(
            (live.event, live.pending_gate),
            (Event::Pending, Gate::Live)
        );
        owner.lease.gate.lock().unwrap().expires_at = Instant::now();
        observer
            .scope(async {
                observe(Phase::ClaimRow, async {}).await;
                exit(Exit::ClaimLateReady);
            })
            .await;
        let late = observer.snapshot();
        assert_eq!(
            (late.event, late.poll_gate, late.exit_gate),
            (Event::Ready, Gate::Expired, Gate::Expired)
        );
        assert_eq!(
            (late.pending_phase, late.pending_gate),
            (Phase::RestoreMaintenance, Gate::Live)
        );
        other.scope(observe(Phase::Body, async {})).await;
        assert_eq!(other.snapshot().phase, Phase::Body);
        assert!(!other.snapshot().saw_pending && other.snapshot().request != late.request);
        assert_eq!(observer.snapshot().phase, Phase::ClaimRow);
        assert!(late.saw_live_pending && late.live_pending_phase == Phase::RestoreMaintenance);
    }
}
