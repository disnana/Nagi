//! Closed request-owned native call boundaries, not OS wait classifications.
use super::{Db, Observer};
use std::{
    fmt,
    sync::atomic::{AtomicU64, Ordering},
};
pub(super) const LIMIT_US: u32 = 60_000_000;
pub(super) const UNSEEN_US: u32 = LIMIT_US + 1;
fn bounded_us(duration: std::time::Duration) -> u32 {
    duration.as_micros().min(u128::from(LIMIT_US)) as u32
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum Stage {
    Unseen = 0,
    CommitEntered = 1,
    CommitOk = 2,
    CommitErr = 3,
    CleanupEntered = 4,
    CleanupOk = 5,
    CleanupErr = 6,
    CheckoutReturned = 7,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Record {
    pub(super) valid: bool,
    active: bool,
    pub(super) stage: Stage,
    pub(super) commit_result: u8,
    pub(super) cleanup_result: u8,
    pub(super) cleanup_reason: u8,
    pub(super) commit_enter_us: u32,
    pub(super) commit_return_us: u32,
    pub(super) cleanup_enter_us: u32,
    pub(super) cleanup_return_us: u32,
    pub(super) settled_us: u32,
}
impl Record {
    pub(super) const fn new() -> Self {
        Self {
            valid: true,
            active: false,
            stage: Stage::Unseen,
            commit_result: 0,
            cleanup_result: 0,
            cleanup_reason: 0,
            commit_enter_us: UNSEEN_US,
            commit_return_us: UNSEEN_US,
            cleanup_enter_us: UNSEEN_US,
            cleanup_return_us: UNSEEN_US,
            settled_us: UNSEEN_US,
        }
    }
}
// Upper32 bits are the nonwrapping, request-local begin counter. Lower32
// bits are the existing bounded elapsed value; out-of-range means unavailable.
const INVALID_US: u32 = u32::MAX;
pub(super) const fn tagged(begin: u16, stamp: u32) -> u64 {
    ((begin as u64) << 32) | stamp as u64
}
fn unpack(word: u64) -> (u64, u32) {
    (word >> 32, word as u32)
}
#[derive(Clone, Copy)]
pub(super) struct CallSnapshot {
    valid: bool,
    state: u8,
    generation: u16,
    entered: u32,
    returned: u32,
    native_state: u8,
    commit_result: u8,
    settled: u32,
    begins: u16,
    commits: u16,
}
impl CallSnapshot {
    pub(super) const UNAVAILABLE: Self = Self {
        valid: false,
        state: 0,
        generation: 0,
        entered: UNSEEN_US,
        returned: UNSEEN_US,
        native_state: 0,
        commit_result: 0,
        settled: UNSEEN_US,
        begins: 0,
        commits: 0,
    };
    pub(super) fn coherent(
        before: super::Record,
        after: super::Record,
        entry: u64,
        returned: u64,
    ) -> Self {
        let (entry_generation, entry) = unpack(entry);
        let (return_generation, returned) = unpack(returned);
        let n = before.native;
        if before != after
            || !n.valid
            || (n.stage == Stage::CommitOk && n.commit_result != 1)
            || (n.stage == Stage::CommitErr && n.commit_result != 2)
            || ((n.stage == Stage::CheckoutReturned) != (n.settled_us <= LIMIT_US))
            || (n.stage != Stage::CheckoutReturned && n.settled_us != UNSEEN_US)
            || before.begins == 0
            || before.begins >= 1023
            || entry_generation != u64::from(before.begins)
            || return_generation != entry_generation
            || entry > UNSEEN_US
            || returned > UNSEEN_US
            || (returned != UNSEEN_US && (entry == UNSEEN_US || returned < entry))
            || (entry != UNSEEN_US && n.commit_enter_us > entry)
        {
            return Self::UNAVAILABLE;
        }
        let state = if entry == UNSEEN_US {
            if returned != UNSEEN_US
                || n.commit_result != 0
                || matches!(n.stage, Stage::CommitOk | Stage::CommitErr)
            {
                return Self::UNAVAILABLE;
            }
            1
        } else if returned == UNSEEN_US {
            if n.stage != Stage::CommitEntered
                || n.commit_result != 0
                || n.commit_return_us != UNSEEN_US
                || n.settled_us != UNSEEN_US
            {
                return Self::UNAVAILABLE;
            }
            2
        } else if n.stage == Stage::CommitEntered {
            if n.commit_result != 0 || n.commit_return_us != UNSEEN_US || n.settled_us != UNSEEN_US
            {
                return Self::UNAVAILABLE;
            }
            3
        } else {
            if !matches!(n.commit_result, 1 | 2)
                || n.stage == Stage::Unseen
                || n.commit_return_us < returned
                || n.commit_return_us > LIMIT_US
                || (n.stage == Stage::CheckoutReturned && n.settled_us < returned)
            {
                return Self::UNAVAILABLE;
            }
            4
        };
        Self {
            valid: true,
            state,
            generation: before.begins,
            entered: entry,
            returned,
            native_state: n.stage as u8,
            commit_result: n.commit_result,
            settled: n.settled_us,
            begins: before.begins,
            commits: before.commits,
        }
    }
}

// A lexical witness borrows the already request-owned Observer. No new task ID,
// callback, authority, native resource, await or cross-request global slot.
// begin mirrors the existing bounded begins counter only, so an old guard's
// lexical Drop cannot clear a later serial transaction after checkout return.
pub(crate) struct NativeFinish<'a> {
    observer: &'a Observer,
    owns: bool,
    begin: u16,
}
impl<'a> NativeFinish<'a> {
    pub(super) fn start(observer: &'a Observer) -> Self {
        let mut owns = false;
        let mut begin = 0;
        observer.update(|r| {
            let invalid = !r.native.valid || r.native.active || r.begins == 1023;
            // Same old BeginRunning counter update, atomically with reset.
            r.db = Db::BeginRunning;
            r.begins = (r.begins + 1).min(1023);
            if invalid {
                r.native.valid = false;
                return;
            }
            r.native = Record::new();
            r.native.active = true;
            owns = true;
            begin = r.begins;
            observer
                .0
                .native_call_enter
                .store(tagged(begin, UNSEEN_US), Ordering::Release);
            observer
                .0
                .native_call_return
                .store(tagged(begin, UNSEEN_US), Ordering::Release);
        });
        Self {
            observer,
            owns,
            begin,
        }
    }
    fn mark(&self, action: impl FnOnce(&mut Record, u32)) {
        if !self.owns {
            return;
        }
        let elapsed = bounded_us(self.observer.0.admitted.elapsed());
        self.observer.update(|r| {
            if r.native.valid {
                if r.native.active && r.begins == self.begin {
                    action(&mut r.native, elapsed);
                } else {
                    r.native.valid = false;
                }
            }
        });
    }
    pub(crate) fn begin_failed(&self) {
        if self.owns {
            self.observer.update(|r| {
                if r.begins == self.begin {
                    r.native.active = false;
                }
            });
        }
    }
    pub(crate) fn commit_enter(&self) {
        self.mark(|r, now| {
            if r.stage != Stage::Unseen {
                r.valid = false;
                return;
            }
            r.stage = Stage::CommitEntered;
            r.commit_enter_us = now;
        });
    }
    fn publish_call(&self, word: &AtomicU64) {
        if !self.owns {
            return;
        }
        let expected = tagged(self.begin, UNSEEN_US);
        let stamp = tagged(self.begin, bounded_us(self.observer.0.admitted.elapsed()));
        if let Err(prior) =
            word.compare_exchange(expected, stamp, Ordering::Release, Ordering::Acquire)
        {
            // A stale lexical owner never overwrites a later begin. Duplicate
            // same-generation publication is poisoned without taking the mutex.
            if unpack(prior).0 == u64::from(self.begin) {
                let _ = word.compare_exchange(
                    prior,
                    tagged(self.begin, INVALID_US),
                    Ordering::Release,
                    Ordering::Acquire,
                );
            }
        }
        #[cfg(test)]
        self.observer.0.native_call_changed.notify_waiters();
    }
    pub(crate) fn call_enter(&self) {
        self.publish_call(&self.observer.0.native_call_enter);
    }
    pub(crate) fn call_return(&self) {
        self.publish_call(&self.observer.0.native_call_return);
    }
    pub(crate) fn commit_return(&self, ok: bool) {
        self.mark(|r, now| {
            if r.stage != Stage::CommitEntered || now < r.commit_enter_us {
                r.valid = false;
                return;
            }
            r.commit_return_us = now;
            r.commit_result = if ok { 1 } else { 2 };
            r.stage = if ok {
                Stage::CommitOk
            } else {
                Stage::CommitErr
            };
        });
    }
    pub(crate) fn cleanup_eof_enter(&self) {
        self.cleanup_enter(true);
    }
    pub(crate) fn cleanup_after_finish_enter(&self) {
        self.cleanup_enter(false);
    }
    fn cleanup_enter(&self, eof: bool) {
        self.mark(|r, now| {
            if r.cleanup_result != 0
                || (eof && r.stage != Stage::Unseen)
                || [r.commit_enter_us, r.commit_return_us]
                    .into_iter()
                    .any(|stamp| stamp != UNSEEN_US && now < stamp)
            {
                r.valid = false;
                return;
            }
            r.cleanup_reason = if eof {
                1
            } else if r.commit_result == 2 {
                2
            } else {
                3
            };
            r.cleanup_result = 1;
            r.cleanup_enter_us = now;
            r.stage = Stage::CleanupEntered;
        });
    }
    pub(crate) fn cleanup_return(&self, ok: bool) {
        self.mark(|r, now| {
            if r.stage != Stage::CleanupEntered || now < r.cleanup_enter_us {
                r.valid = false;
                return;
            }
            r.cleanup_return_us = now;
            r.cleanup_result = if ok { 2 } else { 3 };
            r.stage = if ok {
                Stage::CleanupOk
            } else {
                Stage::CleanupErr
            };
        });
    }
    pub(crate) fn checkout_returned(&self) {
        self.mark(|r, now| {
            if r.settled_us != UNSEEN_US
                || r.stage == Stage::CleanupEntered
                || [
                    r.commit_enter_us,
                    r.commit_return_us,
                    r.cleanup_enter_us,
                    r.cleanup_return_us,
                ]
                .into_iter()
                .any(|stamp| stamp != UNSEEN_US && stamp > now)
            {
                r.valid = false;
                return;
            }
            r.settled_us = now;
            r.stage = Stage::CheckoutReturned;
            r.active = false;
        });
    }
}
impl Drop for NativeFinish<'_> {
    fn drop(&mut self) {
        if self.owns {
            self.observer.update(|r| {
                if r.begins == self.begin {
                    r.native.active = false;
                }
            });
        }
    }
}
#[derive(Clone, Copy)]
#[repr(u8)]
pub(super) enum Origin {
    Session = 0,
    Token = 1,
    Bootstrap = 2,
    #[cfg(test)]
    Private = 3,
}
pub(super) struct Closed {
    snapshot: super::Closed,
    origin: Origin,
    terminal: u8,
}
impl Closed {
    pub(super) fn new(snapshot: super::Closed, origin: Origin, terminal: u8) -> Self {
        Self {
            snapshot,
            origin,
            terminal,
        }
    }
}
impl fmt::Display for Closed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.snapshot.record;
        let n = r.native;
        write!(f,"session_native_finish_diag status=504 origin={} terminal={} diagnostic_valid={} native_state={} commit_result={} cleanup_result={} cleanup_reason={} commit_enter_us={} commit_return_us={} cleanup_enter_us={} cleanup_return_us={} settled_us={} begins={} commits={} selected_deadline_passed={}",
            self.origin as u8,self.terminal,u8::from(n.valid),n.stage as u8,n.commit_result,n.cleanup_result,n.cleanup_reason,n.commit_enter_us,n.commit_return_us,n.cleanup_enter_us,n.cleanup_return_us,n.settled_us,r.begins,r.commits,u8::from(self.snapshot.selected_deadline_passed))
    }
}
pub(super) struct CallClosed {
    snapshot: super::Closed,
    origin: Origin,
    terminal: u8,
}
impl CallClosed {
    pub(super) fn new(snapshot: super::Closed, origin: Origin, terminal: u8) -> Self {
        Self {
            snapshot,
            origin,
            terminal,
        }
    }
}
impl fmt::Display for CallClosed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = self.snapshot.native_call;
        write!(f,"session_native_call_diag status=504 origin={} terminal={} diagnostic_valid={} native_call_state={} generation={} call_enter_us={} call_return_us={} native_state={} commit_result={} settled_us={} begins={} commits={} selected_deadline_passed={}",
            self.origin as u8,self.terminal,u8::from(n.valid),n.state,n.generation,n.entered,n.returned,n.native_state,n.commit_result,n.settled,n.begins,n.commits,u8::from(n.valid && self.snapshot.selected_deadline_passed))
    }
}

#[cfg(test)]
#[path = "session_native_finish_tests.rs"]
mod tests;
