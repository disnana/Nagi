//! Opt-in request-owned closed witness. No headers, SQL, payload, error or path.
//! Native stages describe actual execution boundaries, never inferred lock waits.
use std::{
    fmt,
    future::Future,
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Phase {
    Entry,
    Identity,
    Issue,
    Seal,
    Wire,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Event {
    Unpolled,
    Pending,
    Ready,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Exit {
    Unclassified,
    Timer,
    LateReady,
    Gate,
    Wire,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Db {
    None,
    LogicalCheckout,
    CheckoutReady,
    BeginEnqueue,
    BeginRunning,
    BeginReply,
    ExecRunning,
    ExecReply,
    ReadRunning,
    ReadReply,
    CommitRunning,
    CommitDone,
    RollbackRunning,
    RollbackDone,
    Settled,
}
#[derive(Clone, Copy)]
struct Record {
    phase: Phase,
    event: Event,
    db: Db,
    exit: Exit,
    polls: u16,
    begins: u16,
    commands: u16,
    commits: u16,
    budget_ms: u16,
    entry_remaining_ms: u16,
    phase_remaining_ms: u16,
}
struct State {
    until: Instant,
    record: Mutex<Record>,
}
#[derive(Clone)]
pub(crate) struct Observer(Arc<State>);
tokio::task_local! { static ACTIVE: Observer; }
fn bounded_ms(duration: std::time::Duration) -> u16 {
    duration.as_millis().min(60_000) as u16
}
impl Observer {
    pub(crate) fn new(admitted: Instant, until: Instant) -> Self {
        let remaining = bounded_ms(until.saturating_duration_since(Instant::now()));
        Self(Arc::new(State {
            until,
            record: Mutex::new(Record {
                phase: Phase::Entry,
                event: Event::Unpolled,
                db: Db::None,
                exit: Exit::Unclassified,
                polls: 0,
                begins: 0,
                commands: 0,
                commits: 0,
                budget_ms: bounded_ms(until.saturating_duration_since(admitted)),
                entry_remaining_ms: remaining,
                phase_remaining_ms: remaining,
            }),
        }))
    }
    pub(crate) async fn scope<F: Future>(&self, future: F) -> F::Output {
        ACTIVE.scope(self.clone(), future).await
    }
    fn update(&self, action: impl FnOnce(&mut Record)) {
        if let Ok(mut record) = self.0.record.lock() {
            action(&mut record);
        }
    }
    pub(crate) fn db(&self, stage: Db) {
        self.update(|r| {
            r.db = stage;
            match stage {
                Db::BeginRunning => r.begins = (r.begins + 1).min(1023),
                Db::ExecRunning | Db::ReadRunning => r.commands = (r.commands + 1).min(1023),
                Db::CommitDone => r.commits = (r.commits + 1).min(1023),
                _ => {}
            }
        });
    }
    fn closed(&self, status: u16) -> Option<Closed> {
        if status != 504 {
            return None;
        }
        let record = *self.0.record.lock().ok()?;
        Some(Closed {
            record,
            exit_remaining_ms: bounded_ms(self.0.until.saturating_duration_since(Instant::now())),
        })
    }
    pub(crate) fn emit(&self, status: u16) {
        if let Some(closed) = self.closed(status) {
            eprintln!("{closed}");
        }
    }
}
pub(crate) fn current() -> Option<Observer> {
    ACTIVE.try_with(Clone::clone).ok()
}
pub(crate) fn phase(phase: Phase) {
    let _ = ACTIVE.try_with(|o| {
        let remaining = bounded_ms(o.0.until.saturating_duration_since(Instant::now()));
        o.update(|r| {
            r.phase = phase;
            r.event = Event::Unpolled;
            r.phase_remaining_ms = remaining;
        });
    });
}
pub(crate) fn exit(exit: Exit) {
    let _ = ACTIVE.try_with(|o| o.update(|r| r.exit = exit));
}
pub(crate) async fn observe<F: Future>(stage: Phase, future: F) -> F::Output {
    phase(stage);
    let mut future = std::pin::pin!(future);
    std::future::poll_fn(|cx| {
        let result = future.as_mut().poll(cx);
        let _ = ACTIVE.try_with(|o| {
            o.update(|r| {
                r.event = if result.is_pending() {
                    Event::Pending
                } else {
                    Event::Ready
                };
                r.polls = (r.polls + 1).min(1023);
            })
        });
        result
    })
    .await
}
struct Closed {
    record: Record,
    exit_remaining_ms: u16,
}
impl fmt::Display for Closed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.record;
        write!(f,"session_deadline_diag status=504 phase={} event={} db={} exit={} polls={} begins={} commands={} commits={} budget_ms={} entry_remaining_ms={} phase_remaining_ms={} exit_remaining_ms={}",
            r.phase as u8,r.event as u8,r.db as u8,r.exit as u8,r.polls,r.begins,r.commands,r.commits,r.budget_ms,r.entry_remaining_ms,r.phase_remaining_ms,self.exit_remaining_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{task::Poll, time::Duration};
    #[tokio::test]
    async fn pending_ready_and_bounded_closed_failure_only() {
        let admitted = Instant::now();
        let observer = Observer::new(admitted, admitted + Duration::from_secs(10));
        let mut pending = true;
        let payload = "credential SQL body error /private/path";
        let result = observer
            .scope(observe(
                Phase::Identity,
                std::future::poll_fn(|cx| {
                    if pending {
                        pending = false;
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    } else {
                        Poll::Ready(payload)
                    }
                }),
            ))
            .await;
        assert_eq!(result, payload);
        let record = *observer.0.record.lock().unwrap();
        assert_eq!(record.event, Event::Ready);
        assert_eq!(record.polls, 2);
        assert!(observer.closed(200).is_none());
        assert!(observer.closed(403).is_none());
        let output = observer.closed(504).unwrap().to_string();
        assert!(!output.contains(payload));
        assert!(output.len() < 512);
        assert!(output
            .strip_prefix("session_deadline_diag ")
            .unwrap()
            .split(' ')
            .all(|field| field.split_once('=').is_some_and(
                |(_, value)| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
            )));
        for _ in 0..1100 {
            observer.db(Db::BeginRunning);
            observer.db(Db::ExecRunning);
            observer.db(Db::CommitDone);
        }
        let record = *observer.0.record.lock().unwrap();
        assert_eq!(
            (record.begins, record.commands, record.commits),
            (1023, 1023, 1023)
        );
    }
    #[tokio::test]
    async fn request_scope_isolated_and_native_owner_survives_scope() {
        let admitted = Instant::now();
        let first = Observer::new(admitted, admitted + Duration::from_secs(10));
        let second = Observer::new(admitted, admitted + Duration::from_secs(10));
        let native = first.scope(async { current().unwrap() }).await;
        assert!(current().is_none());
        let thread = std::thread::spawn(move || native.db(Db::BeginRunning));
        second
            .scope(async {
                phase(Phase::Issue);
                exit(Exit::Timer);
            })
            .await;
        thread.join().unwrap();
        assert_eq!(first.0.record.lock().unwrap().db, Db::BeginRunning);
        assert_eq!(first.0.record.lock().unwrap().phase, Phase::Entry);
        assert_eq!(second.0.record.lock().unwrap().db, Db::None);
        assert_eq!(second.0.record.lock().unwrap().phase, Phase::Issue);
    }
}
