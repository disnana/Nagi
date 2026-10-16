//! Fixed, test-only auth stage witness. No credential or database value enters a record.
use super::FailureKind;
use crate::sqlite::Outcome;
use std::{
    fmt::{self, Write as _},
    io::Write as _,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Mutex, OnceLock, TryLockError,
    },
    time::Instant,
};

const LOCAL: usize = 24;
const GLOBAL: usize = 64;
const OUTPUT: usize = 8192;
#[derive(Clone, Copy, Debug)]
pub(super) enum Op {
    CsrfIssue,
    CsrfCheckpoint,
    Bootstrap,
    Claim,
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Stage {
    Entry,
    BeginAck,
    Binding,
    Row,
    Expiry,
    Precommit,
    FinishAck,
    RestoreBefore,
    RestoreAck,
    PostcommitBefore,
    PostcommitAck,
    FinalClock,
    Gate,
    Return,
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Event {
    Seen,
    Match,
    Miss,
    Nominal,
    AbsoluteClamped,
    Passed,
    Refused,
    WallExpired,
    GateExpiredOrEnded,
    MonoExpired,
    Success,
    Failure,
}
#[derive(Clone, Copy)]
enum Mono {
    Before(u64),
    At,
    After(u64),
}
impl fmt::Debug for Mono {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Before(us) => write!(f, "Before({us})"),
            Self::At => f.write_str("At"),
            Self::After(us) => write!(f, "After({us})"),
        }
    }
}
#[derive(Clone, Copy)]
struct Record {
    op: Op,
    stage: Stage,
    event: Event,
    elapsed_us: Option<u64>,
    utc_remaining_ms: Option<i128>,
    mono: Option<Mono>,
    kind: Option<FailureKind>,
    outcome: Option<Outcome>,
}
const EMPTY: Record = Record {
    op: Op::CsrfIssue,
    stage: Stage::Entry,
    event: Event::Seen,
    elapsed_us: None,
    utc_remaining_ms: None,
    mono: None,
    kind: None,
    outcome: None,
};
struct Store {
    records: [Record; GLOBAL],
    len: usize,
}
impl Store {
    const fn new() -> Self {
        Self {
            records: [EMPTY; GLOBAL],
            len: 0,
        }
    }
}
struct Collector {
    start: OnceLock<Instant>,
    store: Mutex<Store>,
    lost: AtomicBool,
}
impl Collector {
    const fn new() -> Self {
        Self {
            start: OnceLock::new(),
            store: Mutex::new(Store::new()),
            lost: AtomicBool::new(false),
        }
    }
    fn merge(&self, records: &[Record], local_lost: bool) {
        if local_lost {
            self.lost.store(true, Ordering::Relaxed);
        }
        match self.store.try_lock() {
            Ok(mut store) => {
                for record in records {
                    if store.len == GLOBAL {
                        self.lost.store(true, Ordering::Relaxed);
                        break;
                    }
                    let at = store.len;
                    store.records[at] = *record;
                    store.len += 1;
                }
            }
            Err(TryLockError::WouldBlock | TryLockError::Poisoned(_)) => {
                self.lost.store(true, Ordering::Relaxed)
            }
        }
    }
}
static SLOTS: [Collector; 2] = [Collector::new(), Collector::new()];
fn collector(slot: u8) -> Option<&'static Collector> {
    SLOTS.get(usize::from(slot.checked_sub(1)?))
}

pub(super) struct Trace {
    slot: u8,
    op: Op,
    records: [Record; LOCAL],
    len: usize,
    lost: bool,
}
impl Trace {
    pub(super) fn new(slot: &AtomicU8, op: Op) -> Self {
        Self {
            slot: slot.load(Ordering::Relaxed),
            op,
            records: [EMPTY; LOCAL],
            len: 0,
            lost: false,
        }
    }
    pub(super) fn record(
        &mut self,
        stage: Stage,
        event: Event,
        utc: Option<(i64, i64)>,
        mono: Option<(Instant, Instant)>,
    ) {
        let Some(collector) = collector(self.slot) else {
            return;
        };
        if self.len == LOCAL {
            self.lost = true;
            return;
        }
        let elapsed_us = collector
            .start
            .get()
            .and_then(|start| Instant::now().checked_duration_since(*start))
            .map(|span| u64::try_from(span.as_micros()).unwrap_or(u64::MAX));
        let mono = mono.map(
            |(deadline, sample)| match deadline.checked_duration_since(sample) {
                Some(d) if d.is_zero() => Mono::At,
                Some(d) => Mono::Before(u64::try_from(d.as_micros()).unwrap_or(u64::MAX)),
                None => Mono::After(
                    u64::try_from(sample.duration_since(deadline).as_micros()).unwrap_or(u64::MAX),
                ),
            },
        );
        self.records[self.len] = Record {
            op: self.op,
            stage,
            event,
            elapsed_us,
            utc_remaining_ms: utc.map(|(expiry, ceil)| i128::from(expiry) - i128::from(ceil)),
            mono,
            kind: None,
            outcome: None,
        };
        self.len += 1;
    }
    // Called only from the existing held-gate expiry branch. This is two local
    // fixed writes: no telemetry clock, global collector access, allocation or I/O.
    pub(super) fn mono_expired_local(&mut self, utc: Option<(i64, i64)>) {
        if !(1..=2).contains(&self.slot) {
            return;
        }
        let remaining = utc.map(|(expiry, ceil)| i128::from(expiry) - i128::from(ceil));
        for (stage, event, kind, outcome) in [
            (Stage::Gate, Event::MonoExpired, None, None),
            (
                Stage::Return,
                Event::Failure,
                Some(FailureKind::Expired),
                Some(Outcome::Committed),
            ),
        ] {
            if self.len == LOCAL {
                self.lost = true;
                break;
            }
            self.records[self.len] = Record {
                op: self.op,
                stage,
                event,
                elapsed_us: None,
                utc_remaining_ms: remaining,
                mono: None,
                kind,
                outcome,
            };
            self.len += 1;
        }
    }
    pub(super) fn returned(
        &mut self,
        failure: Option<(FailureKind, Outcome)>,
        utc: Option<(i64, i64)>,
        mono: Option<(Instant, Instant)>,
    ) {
        let before = self.len;
        self.record(
            Stage::Return,
            if failure.is_some() {
                Event::Failure
            } else {
                Event::Success
            },
            utc,
            mono,
        );
        if self.len > before {
            if let Some((kind, outcome)) = failure {
                let record = &mut self.records[self.len - 1];
                record.kind = Some(kind);
                record.outcome = Some(outcome);
            }
        }
    }
}
impl Drop for Trace {
    fn drop(&mut self) {
        if let Some(collector) = collector(self.slot) {
            collector.merge(&self.records[..self.len], self.lost);
        }
    }
}

pub(super) struct Guard {
    slot: u8,
    emitted: bool,
}
impl Guard {
    pub(super) fn new(slot: u8) -> Self {
        let collector = collector(slot).expect("fixed diagnostic slot");
        let _ = collector.start.set(Instant::now());
        Self {
            slot,
            emitted: false,
        }
    }
    pub(super) fn attach(&self, dest: &AtomicU8) {
        dest.store(self.slot, Ordering::Relaxed);
    }
    pub(super) fn emit(mut self) {
        self.write();
        self.emitted = true;
    }
    fn write(&self) {
        let Some(collector) = collector(self.slot) else {
            return;
        };
        let mut output = FixedWriter::new();
        let _ = writeln!(
            output,
            "auth-stage slot={} (test-only; scheduling perturbed)",
            self.slot
        );
        match collector.store.try_lock() {
            Ok(store) => {
                for record in &store.records[..store.len] {
                    let _ = writeln!(output, "op={:?} stage={:?} event={:?} elapsed_us={:?} utc_remaining_ms={:?} mono={:?} kind={:?} outcome={:?}",
                    record.op, record.stage, record.event, record.elapsed_us, record.utc_remaining_ms,
                    record.mono, record.kind, record.outcome);
                }
            }
            Err(TryLockError::WouldBlock | TryLockError::Poisoned(_)) => {
                collector.lost.store(true, Ordering::Relaxed)
            }
        }
        if output.truncated {
            collector.lost.store(true, Ordering::Relaxed);
        }
        let lost = collector.lost.load(Ordering::Relaxed);
        output.footer(if lost { "lost=true\n" } else { "lost=false\n" });
        let _ = std::io::stderr().write_all(&output.buf[..output.len]);
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        if !self.emitted {
            self.write();
        }
    }
}
struct FixedWriter {
    buf: [u8; OUTPUT],
    len: usize,
    truncated: bool,
}
impl FixedWriter {
    fn new() -> Self {
        Self {
            buf: [0; OUTPUT],
            len: 0,
            truncated: false,
        }
    }
    fn footer(&mut self, value: &str) {
        let end = self.len + value.len();
        self.buf[self.len..end].copy_from_slice(value.as_bytes());
        self.len = end;
    }
}
impl fmt::Write for FixedWriter {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let Some(end) = self.len.checked_add(text.len()) else {
            self.truncated = true;
            return Err(fmt::Error);
        };
        if end > OUTPUT - 64 {
            self.truncated = true;
            return Err(fmt::Error);
        }
        self.buf[self.len..end].copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_storage_loss_and_bounded_output() {
        let slot = AtomicU8::new(1);
        let mut trace = Trace::new(&slot, Op::Claim);
        for _ in 0..LOCAL + 1 {
            trace.record(Stage::Entry, Event::Seen, None, None);
        }
        // The synthetic local overflow must not merge into either live test slot.
        trace.slot = 0;
        assert!(trace.lost && trace.len == LOCAL);
        let branch_slot = AtomicU8::new(2);
        let mut branch = Trace::new(&branch_slot, Op::Claim);
        branch.mono_expired_local(Some((100, 101)));
        branch.slot = 0;
        assert_eq!(branch.len, 2);
        assert!(branch.records[0].elapsed_us.is_none());
        assert!(matches!(branch.records[1].kind, Some(FailureKind::Expired)));
        let disabled = AtomicU8::new(0);
        let mut silent = Trace::new(&disabled, Op::Claim);
        silent.record(Stage::Entry, Event::Seen, None, None);
        assert_eq!(silent.len, 0);
        let collector = Collector::new();
        collector.merge(&[EMPTY; GLOBAL], false);
        collector.merge(&[EMPTY], false);
        assert_eq!(collector.store.lock().unwrap().len, GLOBAL);
        assert!(collector.lost.load(Ordering::Relaxed));
        let mut output = FixedWriter::new();
        assert!(output.write_str(&"x".repeat(OUTPUT + 1)).is_err());
        assert!(output.truncated && output.len == 0);
    }
    #[test]
    fn contended_and_poisoned_merge_only_mark_loss() {
        let collector = Collector::new();
        let hold = collector.store.lock().unwrap();
        collector.merge(&[EMPTY], false);
        assert!(collector.lost.load(Ordering::Relaxed));
        drop(hold);
        let poisoned = std::sync::Arc::new(Collector::new());
        let other = std::sync::Arc::clone(&poisoned);
        let _ = std::thread::spawn(move || {
            let _hold = other.store.lock().unwrap();
            panic!("poison")
        })
        .join();
        poisoned.merge(&[EMPTY], false);
        assert!(poisoned.lost.load(Ordering::Relaxed));
    }
}
