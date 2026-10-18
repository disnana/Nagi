//! Opt-in request-owned closed witness. No headers, SQL, payload, error or path.
//! Native stages describe actual execution boundaries, never inferred lock waits.
use std::{
    fmt,
    future::Future,
    sync::{Arc, Mutex},
    time::Instant,
};

#[path = "session_native_finish.rs"]
mod native_finish;
pub(crate) use native_finish::NativeFinish;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Phase {
    Entry,
    Identity,
    Issue,
    Seal,
    Wire,
    Bootstrap,
    Business,
    Restore,
    Delivery,
    #[cfg(test)]
    Preparation,
    GuardedPreparation = 10,
    GuardedBody = 11,
    GuardedCheckpoint = 12,
    DeferredTouch = 13,
    GuardedHandler = 14,
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
// Token504 terminal branch only. This is distinct from the last polled phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum TokenTerminal {
    Unclassified,
    Constrain,
    LateCompletion,
    LiveGate,
    TakeWire,
    Timer,
    InnerRefusal,
}
// Selected at actual Guarded dispatcher branches, never inferred from Db/phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum GuardedTerminal {
    Unclassified,
    Preparation,
    LiveGate,
    Body,
    CheckpointLateReady,
    CheckpointTimer,
    CheckpointRefusal,
    HandlerLateReady,
    HandlerTimer,
    Constrain,
}
// Unit-only ordinary dispatcher terminal, selected at the actual match arm.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum PrivateTerminal {
    PreparationTimer = 1,
    PreparationLateReady,
    HandlerTimer,
    HandlerLateReady,
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
    until: Instant,
    phase_at: Instant,
    phase: Phase,
    event: Event,
    db: Db,
    exit: Exit,
    token_terminal: TokenTerminal,
    guarded_terminal: Option<GuardedTerminal>,
    polls: u16,
    begins: u16,
    commands: u16,
    commits: u16,
    budget_ms: u16,
    entry_remaining_ms: u16,
    phase_remaining_ms: u16,
    native: native_finish::Record,
}
struct State {
    admitted: Instant,
    entered: Instant,
    security_until: Instant,
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
        let entered = Instant::now();
        let remaining = bounded_ms(until.saturating_duration_since(entered));
        Self(Arc::new(State {
            admitted,
            entered,
            security_until: until,
            record: Mutex::new(Record {
                until,
                phase_at: entered,
                phase: Phase::Entry,
                event: Event::Unpolled,
                db: Db::None,
                exit: Exit::Unclassified,
                token_terminal: TokenTerminal::Unclassified,
                guarded_terminal: None,
                polls: 0,
                begins: 0,
                commands: 0,
                commits: 0,
                budget_ms: bounded_ms(until.saturating_duration_since(admitted)),
                entry_remaining_ms: remaining,
                phase_remaining_ms: remaining,
                native: native_finish::Record::new(),
            }),
        }))
    }
    pub(crate) fn new_guarded(admitted: Instant, until: Instant) -> Self {
        let observer = Self::new(admitted, until);
        observer.update(|r| r.guarded_terminal = Some(GuardedTerminal::Unclassified));
        observer
    }
    pub(crate) async fn scope<F: Future>(&self, future: F) -> F::Output {
        ACTIVE.scope(self.clone(), future).await
    }
    fn update(&self, action: impl FnOnce(&mut Record)) {
        if let Ok(mut record) = self.0.record.lock() {
            action(&mut record);
        }
    }
    pub(crate) fn native_finish(&self) -> NativeFinish<'_> {
        NativeFinish::start(self)
    }
    #[cfg(test)]
    pub(crate) fn native_finish_for_test(&self) -> String {
        native_finish::Closed::new(
            self.closed(504).expect("unpoisoned test observer"),
            native_finish::Origin::Session,
            0,
        )
        .to_string()
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
    // Diagnostic-only mirror of the already selected production bound. It
    // cannot renew either timer; no lease/native/cookie state is changed here.
    fn select_until(&self, until: Instant) {
        self.update(|r| {
            r.until = r.until.min(until);
            r.budget_ms = bounded_ms(r.until.saturating_duration_since(self.0.admitted));
            r.entry_remaining_ms = bounded_ms(r.until.saturating_duration_since(self.0.entered));
            r.phase_remaining_ms = bounded_ms(r.until.saturating_duration_since(r.phase_at));
        });
    }
    fn closed(&self, status: u16) -> Option<Closed> {
        if status != 504 {
            return None;
        }
        let held = self.0.record.lock().ok()?;
        let now = Instant::now();
        let record = *held;
        Some(Closed {
            record,
            exit_remaining_ms: bounded_ms(record.until.saturating_duration_since(now)),
            selected_deadline_passed: now >= record.until,
        })
    }
    pub(crate) fn emit(&self, status: u16) {
        if let Some(closed) = self.closed(status) {
            eprintln!(
                "{closed}\n{}",
                native_finish::Closed::new(closed, native_finish::Origin::Session, 0)
            );
        }
    }
    #[cfg(test)]
    pub(crate) fn emit_private(&self, terminal: PrivateTerminal) {
        // Copy the retained owner's record after selection. The scoped future
        // may already be dropped; ACTIVE is deliberately not consulted here.
        if let Some(closed) = self.closed(504) {
            eprintln!(
                "{}\n{}",
                PrivateClosed { closed, terminal },
                native_finish::Closed::new(closed, native_finish::Origin::Private, terminal as u8)
            );
        }
    }
    pub(crate) fn emit_guarded(&self, status: u16) {
        if let Some(closed) = self.closed(status) {
            eprintln!("{}", GuardedClosed(closed));
        }
    }
    pub(crate) fn emit_token(&self, status: u16) {
        if let Some(closed) = self.closed(status) {
            eprintln!(
                "{}\n{}",
                TokenClosed(closed),
                native_finish::Closed::new(
                    closed,
                    native_finish::Origin::Token,
                    closed.record.token_terminal as u8
                )
            );
        }
    }
    pub(crate) fn emit_bootstrap(&self, status: u16) {
        if let Some(closed) = self.closed(status) {
            let selected_end_kind = u8::from(closed.record.until < self.0.security_until);
            let security_budget_ms = bounded_ms(
                self.0
                    .security_until
                    .saturating_duration_since(self.0.admitted),
            );
            eprintln!(
                "{}\n{}",
                BootstrapClosed {
                    closed,
                    security_budget_ms,
                    selected_end_kind
                },
                native_finish::Closed::new(closed, native_finish::Origin::Bootstrap, 0)
            );
        }
    }
}
pub(crate) fn current() -> Option<Observer> {
    ACTIVE.try_with(Clone::clone).ok()
}
pub(crate) fn phase(phase: Phase) {
    let _ = ACTIVE.try_with(|o| {
        let now = Instant::now();
        o.update(|r| {
            r.phase = phase;
            r.phase_at = now;
            r.event = Event::Unpolled;
            r.phase_remaining_ms = bounded_ms(r.until.saturating_duration_since(now));
        });
    });
}
pub(crate) fn select_until(until: Instant) {
    let _ = ACTIVE.try_with(|o| o.select_until(until));
}
pub(crate) fn exit(exit: Exit) {
    let _ = ACTIVE.try_with(|o| o.update(|r| r.exit = exit));
}
pub(crate) fn token_terminal(terminal: TokenTerminal) {
    let _ = ACTIVE.try_with(|o| o.update(|r| r.token_terminal = terminal));
}
pub(crate) fn guarded_terminal(terminal: GuardedTerminal) {
    let _ = ACTIVE.try_with(|o| o.update(|r| r.guarded_terminal = Some(terminal)));
}
// Shared caller sites also serve ordinary/Browser routes. Leave their existing
// observer phases untouched: only the explicit Guarded scope enables these marks.
pub(crate) async fn observe_guarded<F: Future>(stage: Phase, future: F) -> F::Output {
    let enabled = ACTIVE
        .try_with(|o| {
            o.0.record
                .lock()
                .map(|r| r.guarded_terminal.is_some())
                .unwrap_or(false)
        })
        .unwrap_or(false);
    if enabled {
        observe(stage, future).await
    } else {
        future.await
    }
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
#[derive(Clone, Copy)]
struct Closed {
    record: Record,
    exit_remaining_ms: u16,
    selected_deadline_passed: bool,
}
impl fmt::Display for Closed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.record;
        write!(f,"session_deadline_diag status=504 phase={} event={} db={} exit={} polls={} begins={} commands={} commits={} budget_ms={} entry_remaining_ms={} phase_remaining_ms={} exit_remaining_ms={}",
            r.phase as u8,r.event as u8,r.db as u8,r.exit as u8,r.polls,r.begins,r.commands,r.commits,r.budget_ms,r.entry_remaining_ms,r.phase_remaining_ms,self.exit_remaining_ms)
    }
}
// Only fixed keys and bounded enum/counter/timing values. The native Db value
// is the last observed actual boundary; it is not a claim about lock ownership.
struct GuardedClosed(Closed);
impl fmt::Display for GuardedClosed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.0.record;
        write!(f, "csrf_guarded_deadline_diag status=504 phase={} event={} db={} exit={} polls={} begins={} commands={} commits={} budget_ms={} entry_remaining_ms={} phase_remaining_ms={} exit_remaining_ms={} selected_deadline_passed={} terminal={}",
            r.phase as u8, r.event as u8, r.db as u8, r.exit as u8, r.polls, r.begins,
            r.commands, r.commits, r.budget_ms, r.entry_remaining_ms, r.phase_remaining_ms,
            self.0.exit_remaining_ms, u8::from(self.0.selected_deadline_passed), r.guarded_terminal.unwrap_or(GuardedTerminal::Unclassified) as u8)
    }
}
#[cfg(test)]
struct PrivateClosed {
    closed: Closed,
    terminal: PrivateTerminal,
}
#[cfg(test)]
impl fmt::Display for PrivateClosed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.closed.record;
        write!(f,"session_private_deadline_diag status=504 phase={} event={} db={} polls={} begins={} commands={} commits={} budget_ms={} entry_remaining_ms={} phase_remaining_ms={} exit_remaining_ms={} terminal={}",
            r.phase as u8,r.event as u8,r.db as u8,r.polls,r.begins,r.commands,r.commits,r.budget_ms,r.entry_remaining_ms,r.phase_remaining_ms,self.closed.exit_remaining_ms,self.terminal as u8)
    }
}
// Reuses the existing request-owned phase/event/native snapshot. No new
// polling, SQL hook, clock sample or authority decision is introduced.
struct TokenClosed(Closed);
impl fmt::Display for TokenClosed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.0.record;
        write!(f,"csrf_token_deadline_diag status=504 phase={} event={} db={} exit={} polls={} begins={} commands={} commits={} budget_ms={} entry_remaining_ms={} phase_remaining_ms={} exit_remaining_ms={} terminal={}",
            r.phase as u8,r.event as u8,r.db as u8,r.exit as u8,r.polls,r.begins,r.commands,r.commits,r.budget_ms,r.entry_remaining_ms,r.phase_remaining_ms,self.0.exit_remaining_ms,r.token_terminal as u8)
    }
}
struct BootstrapClosed {
    closed: Closed,
    security_budget_ms: u16,
    selected_end_kind: u8,
}
impl fmt::Display for BootstrapClosed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.closed.record;
        write!(f, "prelogin_bootstrap_deadline_diag status=504 phase={} event={} db={} exit={} polls={} begins={} commands={} commits={} budget_ms={} entry_remaining_ms={} phase_remaining_ms={} exit_remaining_ms={} security_budget_ms={} selected_end_kind={}",
            r.phase as u8, r.event as u8, r.db as u8, r.exit as u8,
            r.polls, r.begins, r.commands, r.commits, r.budget_ms,
            r.entry_remaining_ms, r.phase_remaining_ms, self.closed.exit_remaining_ms,
            self.security_budget_ms, self.selected_end_kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{task::Poll, time::Duration};
    #[test]
    fn guarded_closed_fields_are_numeric_and_terminal_is_independent_of_last_db() {
        let admitted = Instant::now() - Duration::from_secs(2);
        let observer = Observer::new_guarded(admitted, admitted + Duration::from_secs(1));
        observer.update(|r| {
            r.phase = Phase::DeferredTouch;
            r.event = Event::Pending;
            r.db = Db::CommitDone;
            r.guarded_terminal = Some(GuardedTerminal::HandlerTimer);
        });
        assert!(observer.closed(200).is_none());
        assert!(observer.closed(403).is_none());
        let output = GuardedClosed(observer.closed(504).unwrap()).to_string();
        assert!(output.len() < 512);
        let mut fields = output
            .strip_prefix("csrf_guarded_deadline_diag ")
            .unwrap()
            .split(' ');
        for key in [
            "status",
            "phase",
            "event",
            "db",
            "exit",
            "polls",
            "begins",
            "commands",
            "commits",
            "budget_ms",
            "entry_remaining_ms",
            "phase_remaining_ms",
            "exit_remaining_ms",
            "selected_deadline_passed",
            "terminal",
        ] {
            let (actual, value) = fields.next().unwrap().split_once('=').unwrap();
            assert_eq!(actual, key);
            assert!(!value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()));
        }
        assert!(fields.next().is_none());
        assert!(output.contains("phase=13 event=1 db=11 "));
        assert!(output.ends_with("selected_deadline_passed=1 terminal=8"));
        assert_eq!(Phase::GuardedPreparation as u8, 10);
        assert_eq!(Phase::GuardedBody as u8, 11);
        assert_eq!(Phase::GuardedCheckpoint as u8, 12);
        assert_eq!(Phase::GuardedHandler as u8, 14);
    }
    #[tokio::test]
    async fn guarded_scope_tracks_real_polls_and_preserves_non_guarded_phases() {
        let start = Instant::now();
        let until = start + Duration::from_secs(10);
        let ordinary = Observer::new(start, until);
        assert_eq!(
            ordinary
                .scope(observe_guarded(Phase::DeferredTouch, std::future::ready(7)))
                .await,
            7
        );
        assert_eq!(ordinary.0.record.lock().unwrap().phase, Phase::Entry);
        let observer = Observer::new_guarded(start, until);
        let result = observer
            .scope(async {
                let checkpoint =
                    observe_guarded(Phase::GuardedCheckpoint, std::future::ready(1)).await;
                let mut pending = true;
                let touch = observe_guarded(
                    Phase::DeferredTouch,
                    std::future::poll_fn(|cx| {
                        if pending {
                            pending = false;
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        } else {
                            Poll::Ready(2)
                        }
                    }),
                )
                .await;
                observe_guarded(
                    Phase::GuardedHandler,
                    std::future::ready((checkpoint, touch)),
                )
                .await
            })
            .await;
        assert_eq!(result, (1, 2));
        let record = *observer.0.record.lock().unwrap();
        assert_eq!(record.phase, Phase::GuardedHandler);
        assert_eq!(record.event, Event::Ready);
        assert_eq!(record.polls, 4);
        assert_eq!(record.db, Db::None);
        assert_eq!(record.guarded_terminal, Some(GuardedTerminal::Unclassified));
    }
    #[tokio::test]
    async fn guarded_scope_cancellation_drops_inner_owner_without_inventing_terminal() {
        struct Held<'a>(&'a std::sync::atomic::AtomicUsize);
        impl Drop for Held<'_> {
            fn drop(&mut self) {
                self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let drops = std::sync::atomic::AtomicUsize::new(0);
        let start = Instant::now();
        let observer = Observer::new_guarded(start, start + Duration::from_secs(10));
        {
            let mut future =
                std::pin::pin!(observer.scope(observe_guarded(Phase::DeferredTouch, async {
                    let _held = Held(&drops);
                    std::future::pending::<()>().await;
                })));
            std::future::poll_fn(|cx| {
                assert!(future.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            })
            .await;
            assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 0);
        }
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 1);
        let record = *observer.0.record.lock().unwrap();
        assert_eq!(record.phase, Phase::DeferredTouch);
        assert_eq!(record.event, Event::Pending);
        assert_eq!(record.guarded_terminal, Some(GuardedTerminal::Unclassified));
    }
    #[test]
    fn bootstrap_selected_bound_is_finite_and_never_renews_security() {
        let admitted = Instant::now();
        let security = admitted + Duration::from_secs(10);
        let observer = Observer::new(admitted, security);
        let handler = admitted + Duration::from_secs(2);
        observer.select_until(handler);
        observer.select_until(security + Duration::from_secs(10));
        let record = *observer.0.record.lock().unwrap();
        assert_eq!(record.until, handler);
        assert_eq!(record.budget_ms, 2000);
        assert!(record.phase_remaining_ms <= record.entry_remaining_ms);
        assert!(record.entry_remaining_ms <= record.budget_ms);
        assert!(observer.closed(200).is_none());
        let output = BootstrapClosed {
            closed: observer.closed(504).unwrap(),
            security_budget_ms: 10000,
            selected_end_kind: 1,
        }
        .to_string();
        assert!(output.len() < 512);
        assert!(output
            .strip_prefix("prelogin_bootstrap_deadline_diag ")
            .unwrap()
            .split(' ')
            .all(|field| field.split_once('=').is_some_and(
                |(_, value)| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
            )));
    }
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
    async fn private_snapshot_survives_scope_without_cross_request_state() {
        let admitted = Instant::now();
        let first = Observer::new(admitted, admitted + Duration::from_secs(2));
        let second = Observer::new(admitted, admitted + Duration::from_secs(2));
        tokio::join!(
            first.scope(observe(Phase::Preparation, async {})),
            second.scope(observe(Phase::Business, async {}))
        );
        assert!(current().is_none());
        let copied = PrivateClosed {
            closed: first.closed(504).unwrap(),
            terminal: PrivateTerminal::PreparationLateReady,
        };
        first.update(|record| record.phase = Phase::Entry);
        assert_eq!(copied.closed.record.phase, Phase::Preparation);
        assert_eq!(copied.closed.record.event, Event::Ready);
        assert_eq!(second.closed(504).unwrap().record.phase, Phase::Business);
        for status in [200, 400, 401, 408, 500] {
            assert!(second.closed(status).is_none());
        }
        let output = copied.to_string();
        assert!(output.starts_with("session_private_deadline_diag status=504 phase=9 event=2 "));
        assert!(output.ends_with(" terminal=2"));
        assert!(output.len() < 512);
        assert!(output
            .strip_prefix("session_private_deadline_diag ")
            .unwrap()
            .split(' ')
            .all(|field| field.split_once('=').is_some_and(
                |(_, value)| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
            )));
    }
    #[tokio::test]
    async fn token_terminal_is_request_owned_closed_and_504_only() {
        let admitted = Instant::now();
        let first = Observer::new(admitted, admitted + Duration::from_secs(10));
        let second = Observer::new(admitted, admitted + Duration::from_secs(10));
        first
            .scope(async {
                phase(Phase::Issue);
                exit(Exit::Timer);
                token_terminal(TokenTerminal::Timer);
            })
            .await;
        assert_eq!(
            first.0.record.lock().unwrap().token_terminal,
            TokenTerminal::Timer
        );
        assert_eq!(
            second.0.record.lock().unwrap().token_terminal,
            TokenTerminal::Unclassified
        );
        assert!(first.closed(200).is_none());
        assert!(first.closed(403).is_none());
        assert!(first.closed(500).is_none());
        let output = TokenClosed(first.closed(504).unwrap()).to_string();
        assert!(output.starts_with("csrf_token_deadline_diag status=504 phase=2 "));
        assert!(output.ends_with(" terminal=5"));
        assert!(output.len() < 512);
        assert!(output
            .strip_prefix("csrf_token_deadline_diag ")
            .unwrap()
            .split(' ')
            .all(|field| field.split_once('=').is_some_and(
                |(_, value)| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
            )));
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
