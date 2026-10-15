//! Private fixed-table token-bucket core for SF07.
//!
//! No request text is retained. Subject keys are already-verified domain and
//! integer identities supplied by a future integration owner.

use std::convert::TryFrom;
use std::mem::size_of;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

const MAX_RULES: usize = (u16::MAX as usize) + 1;
const MAX_SUBJECT_SLOTS: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuleId(u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DomainId(u16);

impl DomainId {
    /// Called only by trusted startup configuration, never parsed from a request.
    pub(crate) const fn from_trusted_config(index: u16) -> Self {
        Self(index)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FixedKey {
    Route,
    Subject { domain: DomainId, subject: i64 },
}

impl FixedKey {
    pub(crate) const fn subject(domain: DomainId, subject: i64) -> Self {
        Self::Subject { domain, subject }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Slots {
    NoSubject,
    Subject(usize),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Rule {
    burst: u32,
    refill_ns: u64,
    subject_slots: Slots,
    idle_ns: u64,
}

impl Rule {
    pub(crate) const fn new(
        burst: u32,
        refill_ns: u64,
        subject_slots: Slots,
        idle_ns: u64,
    ) -> Self {
        Self {
            burst,
            refill_ns,
            subject_slots,
            idle_ns,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StartupError {
    Invalid,
    Allocation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RateDecision {
    Charged,
    Limited { retry_ns: u64 },
    Capacity,
    Closing,
    ClockFault,
    Fault,
}

#[derive(Clone, Copy)]
enum RuleKind {
    Route,
    Subject,
}

#[derive(Clone, Copy)]
struct Bucket {
    tokens: u32,
    anchor: u64,
    last_charge: u64,
}

#[derive(Clone, Copy)]
struct SubjectEntry {
    key: (DomainId, i64),
    bucket: Bucket,
}

struct RuleState {
    burst: u32,
    refill_ns: u64,
    idle_ns: u64,
    kind: RuleKind,
    route: Option<Bucket>,
    subjects: Vec<Option<SubjectEntry>>,
}

struct RateState {
    rules: Vec<RuleState>,
    last_now: Option<u64>,
    clock_fault: bool,
    faulted: bool,
    closing: bool,
}

pub(crate) struct RateOwner {
    state: Mutex<RateState>,
    origin: Instant,
    rule_count: usize,
}

impl RateOwner {
    pub(crate) fn checked_rate(
        rules: &[Rule],
        retained_byte_cap: usize,
    ) -> Result<Self, StartupError> {
        if rules.is_empty() || rules.len() > MAX_RULES || retained_byte_cap == 0 {
            return Err(StartupError::Invalid);
        }

        let mut total_subject_slots = 0usize;
        for rule in rules {
            if rule.burst == 0 || rule.refill_ns == 0 || rule.idle_ns == 0 {
                return Err(StartupError::Invalid);
            }
            if let Slots::Subject(count) = rule.subject_slots {
                if count == 0 || count > MAX_SUBJECT_SLOTS {
                    return Err(StartupError::Invalid);
                }
                total_subject_slots = total_subject_slots
                    .checked_add(count)
                    .ok_or(StartupError::Invalid)?;
                if total_subject_slots > MAX_SUBJECT_SLOTS {
                    return Err(StartupError::Invalid);
                }
            }
        }

        let rule_bytes = rules
            .len()
            .checked_mul(size_of::<RuleState>())
            .ok_or(StartupError::Invalid)?;
        let slot_bytes = total_subject_slots
            .checked_mul(size_of::<Option<SubjectEntry>>())
            .ok_or(StartupError::Invalid)?;
        let minimum_bytes = size_of::<Self>()
            .checked_add(rule_bytes)
            .and_then(|bytes| bytes.checked_add(slot_bytes))
            .ok_or(StartupError::Invalid)?;
        if minimum_bytes > retained_byte_cap || minimum_bytes > isize::MAX as usize {
            return Err(StartupError::Invalid);
        }

        let mut states = Vec::new();
        states
            .try_reserve_exact(rules.len())
            .map_err(|_| StartupError::Allocation)?;
        for rule in rules {
            let (kind, slot_count) = match rule.subject_slots {
                Slots::NoSubject => (RuleKind::Route, 0),
                Slots::Subject(count) => (RuleKind::Subject, count),
            };
            let mut subjects = Vec::new();
            if slot_count > 0 {
                subjects
                    .try_reserve_exact(slot_count)
                    .map_err(|_| StartupError::Allocation)?;
                subjects.resize_with(slot_count, || None);
            }
            states.push(RuleState {
                burst: rule.burst,
                refill_ns: rule.refill_ns,
                idle_ns: rule.idle_ns,
                kind,
                route: None,
                subjects,
            });
        }

        // Include actual Vec capacities in the retained bound before publishing.
        let actual_bytes = size_of::<Self>()
            .checked_add(
                states
                    .capacity()
                    .checked_mul(size_of::<RuleState>())
                    .ok_or(StartupError::Invalid)?,
            )
            .and_then(|bytes| {
                states.iter().try_fold(bytes, |total, rule| {
                    rule.subjects
                        .capacity()
                        .checked_mul(size_of::<Option<SubjectEntry>>())
                        .and_then(|allocated| total.checked_add(allocated))
                })
            })
            .ok_or(StartupError::Invalid)?;
        if actual_bytes > retained_byte_cap || actual_bytes > isize::MAX as usize {
            return Err(StartupError::Invalid);
        }

        Ok(Self {
            state: Mutex::new(RateState {
                rules: states,
                last_now: None,
                clock_fault: false,
                faulted: false,
                closing: false,
            }),
            origin: Instant::now(),
            rule_count: rules.len(),
        })
    }

    /// Rule IDs are created only as indexes into this checked immutable catalog.
    pub(crate) fn rule_id(&self, index: usize) -> Option<RuleId> {
        if index < self.rule_count && index <= u16::MAX as usize {
            Some(RuleId(index as u16))
        } else {
            None
        }
    }

    /// Samples the monotonic clock after acquiring the transition mutex.
    pub(crate) fn charge(&self, rule: RuleId, key: FixedKey) -> RateDecision {
        self.charge_with_clock(rule, key, || {
            Instant::now()
                .checked_duration_since(self.origin)
                .and_then(checked_elapsed_ns)
        })
    }

    fn charge_with_clock<F>(&self, rule: RuleId, key: FixedKey, sample: F) -> RateDecision
    where
        F: FnOnce() -> Option<u64>,
    {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state.faulted = true;
                return RateDecision::Fault;
            }
        };
        if state.faulted {
            return RateDecision::Fault;
        }
        if state.clock_fault {
            return RateDecision::ClockFault;
        }
        if state.closing {
            return RateDecision::Closing;
        }

        let now = match sample() {
            Some(now) => now,
            None => {
                state.clock_fault = true;
                return RateDecision::ClockFault;
            }
        };
        state.charge_at(rule, key, now)
    }

    #[cfg(test)]
    fn charge_at_with_test_clock(&self, rule: RuleId, key: FixedKey, now: u64) -> RateDecision {
        self.charge_with_clock(rule, key, || Some(now))
    }

    pub(crate) fn close(&self) {
        match self.state.lock() {
            Ok(mut state) => state.closing = true,
            Err(poisoned) => {
                let mut state: std::sync::MutexGuard<'_, RateState> =
                    PoisonError::into_inner(poisoned);
                state.faulted = true;
            }
        }
    }
}

fn checked_elapsed_ns(elapsed: Duration) -> Option<u64> {
    u64::try_from(elapsed.as_nanos()).ok()
}

#[derive(Clone, Copy)]
enum TransitionError {
    Clock,
    Fault,
}

impl RateState {
    fn charge_at(&mut self, rule_id: RuleId, key: FixedKey, now: u64) -> RateDecision {
        if self.faulted {
            return RateDecision::Fault;
        }
        if self.clock_fault {
            return RateDecision::ClockFault;
        }
        if self.closing {
            return RateDecision::Closing;
        }
        if self.last_now.map(|last| now < last).unwrap_or(false) {
            self.clock_fault = true;
            return RateDecision::ClockFault;
        }
        let index = usize::from(rule_id.0);
        if index >= self.rules.len() {
            self.faulted = true;
            return RateDecision::Fault;
        }
        self.last_now = Some(now);

        let result = charge_rule(&mut self.rules[index], key, now);
        match result {
            Ok(decision) => decision,
            Err(TransitionError::Clock) => {
                self.clock_fault = true;
                RateDecision::ClockFault
            }
            Err(TransitionError::Fault) => {
                self.faulted = true;
                RateDecision::Fault
            }
        }
    }

    #[cfg(test)]
    fn charge_at_for_test(&mut self, rule: RuleId, key: FixedKey, now: u64) -> RateDecision {
        self.charge_at(rule, key, now)
    }

    #[cfg(test)]
    fn used_subject_slots(&self, rule: RuleId) -> usize {
        self.rules
            .get(usize::from(rule.0))
            .map(|state| state.subjects.iter().filter(|slot| slot.is_some()).count())
            .unwrap_or(0)
    }

    #[cfg(test)]
    fn subject_capacity(&self, rule: RuleId) -> usize {
        self.rules
            .get(usize::from(rule.0))
            .map(|state| state.subjects.capacity())
            .unwrap_or(0)
    }
}

fn charge_rule(
    rule: &mut RuleState,
    key: FixedKey,
    now: u64,
) -> Result<RateDecision, TransitionError> {
    match (rule.kind, key) {
        (RuleKind::Route, FixedKey::Route) => {
            if rule.route.is_none() {
                rule.route = Some(Bucket {
                    tokens: rule.burst,
                    anchor: now,
                    last_charge: now,
                });
            }
            charge_bucket(
                rule.route.as_mut().ok_or(TransitionError::Fault)?,
                rule.burst,
                rule.refill_ns,
                now,
            )
        }
        (RuleKind::Subject, FixedKey::Subject { domain, subject }) => {
            let key = (domain, subject);
            if let Some(entry) = rule
                .subjects
                .iter_mut()
                .filter_map(Option::as_mut)
                .find(|entry| entry.key == key)
            {
                return charge_bucket(&mut entry.bucket, rule.burst, rule.refill_ns, now);
            }

            if let Some(index) = rule.subjects.iter().position(Option::is_none) {
                rule.subjects[index] = Some(SubjectEntry {
                    key,
                    bucket: Bucket {
                        tokens: rule.burst,
                        anchor: now,
                        last_charge: now,
                    },
                });
                let entry = rule.subjects[index]
                    .as_mut()
                    .ok_or(TransitionError::Fault)?;
                return charge_bucket(&mut entry.bucket, rule.burst, rule.refill_ns, now);
            }

            let mut reusable = None;
            for (index, candidate) in rule.subjects.iter_mut().enumerate() {
                let Some(entry) = candidate.as_mut() else {
                    continue;
                };
                refill_bucket(&mut entry.bucket, rule.burst, rule.refill_ns, now)?;
                let idle = now
                    .checked_sub(entry.bucket.last_charge)
                    .ok_or(TransitionError::Clock)?;
                if entry.bucket.tokens == rule.burst && idle >= rule.idle_ns && reusable.is_none() {
                    reusable = Some(index);
                }
            }

            let Some(index) = reusable else {
                return Ok(RateDecision::Capacity);
            };
            rule.subjects[index] = Some(SubjectEntry {
                key,
                bucket: Bucket {
                    tokens: rule.burst,
                    anchor: now,
                    last_charge: now,
                },
            });
            let entry = rule.subjects[index]
                .as_mut()
                .ok_or(TransitionError::Fault)?;
            charge_bucket(&mut entry.bucket, rule.burst, rule.refill_ns, now)
        }
        _ => Err(TransitionError::Fault),
    }
}

fn charge_bucket(
    bucket: &mut Bucket,
    burst: u32,
    refill_ns: u64,
    now: u64,
) -> Result<RateDecision, TransitionError> {
    refill_bucket(bucket, burst, refill_ns, now)?;
    if bucket.tokens == 0 {
        let elapsed = now
            .checked_sub(bucket.anchor)
            .ok_or(TransitionError::Clock)?;
        let remainder = elapsed % refill_ns;
        let retry_ns = refill_ns
            .checked_sub(remainder)
            .filter(|retry| *retry > 0)
            .ok_or(TransitionError::Fault)?;
        return Ok(RateDecision::Limited { retry_ns });
    }

    bucket.tokens -= 1;
    bucket.last_charge = now;
    Ok(RateDecision::Charged)
}

fn refill_bucket(
    bucket: &mut Bucket,
    burst: u32,
    refill_ns: u64,
    now: u64,
) -> Result<(), TransitionError> {
    if bucket.tokens > burst || refill_ns == 0 {
        return Err(TransitionError::Fault);
    }
    let elapsed = now
        .checked_sub(bucket.anchor)
        .ok_or(TransitionError::Clock)?;
    let periods = elapsed / refill_ns;
    if periods == 0 {
        return Ok(());
    }

    let missing = u64::from(burst - bucket.tokens);
    if periods >= missing {
        bucket.tokens = burst;
    } else {
        bucket.tokens += periods as u32;
    }
    let remainder = elapsed % refill_ns;
    bucket.anchor = now.checked_sub(remainder).ok_or(TransitionError::Fault)?;
    Ok(())
}

pub(crate) fn bridge_charge(o: &RateOwner, d: u16, v: i64, t: u64) -> (u8, u64) {
    let answer =
        o.charge_at_with_test_clock(o.rule_id(0).unwrap(), FixedKey::subject(DomainId(d), v), t);
    match answer {
        RateDecision::Charged => (0, 0),
        RateDecision::Limited { retry_ns } => (1, retry_ns),
        RateDecision::Capacity => (2, 0),
        RateDecision::Closing => (3, 0),
        RateDecision::ClockFault => (4, 0),
        RateDecision::Fault => (5, 0),
    }
}
pub(crate) fn bridge_keys(o: &RateOwner) -> Vec<Option<(u16, i64)>> {
    o.state.lock().unwrap().rules[0]
        .subjects
        .iter()
        .map(|e| e.map(|e| (e.key.0 .0, e.key.1)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{DomainId, FixedKey, RateDecision, RateOwner, Rule, Slots};
    use std::cell::Cell;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Barrier, TryLockError};
    use std::time::Duration;

    fn owner(burst: u32, refill_ns: u64, slots: Slots, idle_ns: u64) -> RateOwner {
        RateOwner::checked_rate(&[Rule::new(burst, refill_ns, slots, idle_ns)], 1_000_000)
            .expect("valid fixed rules")
    }

    fn charge_at(owner: &RateOwner, rule: super::RuleId, key: FixedKey, now: u64) -> RateDecision {
        let mut state = owner.state.lock().expect("unpoisoned test state");
        state.charge_at_for_test(rule, key, now)
    }

    fn subject(domain: u16, value: i64) -> FixedKey {
        FixedKey::subject(DomainId::from_trusted_config(domain), value)
    }

    #[test]
    fn r01_burst_two_refills_exactly_one_token_per_interval() {
        let owner = owner(2, 10, Slots::NoSubject, 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        let actual = [
            charge_at(&owner, rule, FixedKey::Route, 0),
            charge_at(&owner, rule, FixedKey::Route, 0),
            charge_at(&owner, rule, FixedKey::Route, 0),
            charge_at(&owner, rule, FixedKey::Route, 10),
            charge_at(&owner, rule, FixedKey::Route, 10),
        ];
        assert_eq!(
            actual,
            [
                RateDecision::Charged,
                RateDecision::Charged,
                RateDecision::Limited { retry_ns: 10 },
                RateDecision::Charged,
                RateDecision::Limited { retry_ns: 10 },
            ]
        );
    }

    #[test]
    fn r06_depleted_subject_debt_cannot_be_evicted() {
        let owner = owner(2, 100, Slots::Subject(1), 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 2), 2),
            RateDecision::Capacity
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 2),
            RateDecision::Limited { retry_ns: 98 }
        );
    }

    #[test]
    fn r07_full_refill_and_idle_allow_one_fixed_slot_reuse() {
        let owner = owner(2, 10, Slots::Subject(1), 5);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 2), 20),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 2), 20),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 2), 20),
            RateDecision::Limited { retry_ns: 10 }
        );
        let state = owner.state.lock().expect("unpoisoned test state");
        assert_eq!(state.used_subject_slots(rule), 1);
        assert_eq!(state.subject_capacity(rule), 1);
    }

    #[test]
    fn r08_partial_refill_is_not_evicted_or_reset() {
        let owner = owner(2, 10, Slots::Subject(1), 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 2), 10),
            RateDecision::Capacity
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 10),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 10),
            RateDecision::Limited { retry_ns: 10 }
        );
    }

    #[test]
    fn r11_subject_table_stays_fixed_and_refuses_untracked_subjects() {
        let owner = owner(1, 10, Slots::Subject(2), 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&owner, rule, subject(1, 1), 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 2), 0),
            RateDecision::Charged
        );
        let before = {
            let state = owner.state.lock().expect("unpoisoned test state");
            state.subject_capacity(rule)
        };
        assert_eq!(
            charge_at(&owner, rule, subject(1, 3), 0),
            RateDecision::Capacity
        );
        let state = owner.state.lock().expect("unpoisoned test state");
        assert_eq!(state.subject_capacity(rule), before);
        assert_eq!(state.used_subject_slots(rule), 2);
    }

    #[test]
    fn production_clock_path_and_close_are_usable() {
        let owner = owner(1, 1_000_000_000_000, Slots::NoSubject, 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(owner.charge(rule, FixedKey::Route), RateDecision::Charged);
        assert!(matches!(
            owner.charge(rule, FixedKey::Route),
            RateDecision::Limited { retry_ns } if retry_ns > 0 && retry_ns <= 1_000_000_000_000
        ));
        owner.close();
        assert_eq!(owner.charge(rule, FixedKey::Route), RateDecision::Closing);
    }

    #[test]
    fn r02_refill_boundary_minus_one_then_exact_interval() {
        let owner = owner(1, 10, Slots::NoSubject, 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&owner, rule, FixedKey::Route, 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, FixedKey::Route, 9),
            RateDecision::Limited { retry_ns: 1 }
        );
        assert_eq!(
            charge_at(&owner, rule, FixedKey::Route, 10),
            RateDecision::Charged
        );
    }

    #[test]
    fn r03_huge_elapsed_saturates_before_token_subtraction() {
        let huge_elapsed_owner = owner(2, 1, Slots::NoSubject, 1);
        let rule = huge_elapsed_owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&huge_elapsed_owner, rule, FixedKey::Route, 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&huge_elapsed_owner, rule, FixedKey::Route, u64::MAX),
            RateDecision::Charged
        );
        let state = huge_elapsed_owner
            .state
            .lock()
            .expect("unpoisoned test state");
        let bucket = state.rules[0].route.expect("route bucket retained");
        assert_eq!(bucket.tokens, 1);
        assert_eq!(bucket.anchor, u64::MAX);
        assert_eq!(bucket.last_charge, u64::MAX);

        let long_interval = owner(1, u64::MAX, Slots::NoSubject, 1);
        let long_rule = long_interval.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&long_interval, long_rule, FixedKey::Route, 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&long_interval, long_rule, FixedKey::Route, 0),
            RateDecision::Limited { retry_ns: u64::MAX }
        );
        assert_eq!(
            charge_at(&long_interval, long_rule, FixedKey::Route, u64::MAX),
            RateDecision::Charged
        );
    }

    #[test]
    fn r04_zero_and_oversized_rule_settings_fail_startup() {
        assert!(
            RateOwner::checked_rate(&[Rule::new(0, 1, Slots::NoSubject, 1)], 1_000_000).is_err()
        );
        assert!(
            RateOwner::checked_rate(&[Rule::new(1, 0, Slots::NoSubject, 1)], 1_000_000).is_err()
        );
        assert!(
            RateOwner::checked_rate(&[Rule::new(1, 1, Slots::Subject(0), 1)], 1_000_000).is_err()
        );
        assert!(
            RateOwner::checked_rate(&[Rule::new(1, 1, Slots::Subject(4097), 1)], 1_000_000)
                .is_err()
        );
        assert!(RateOwner::checked_rate(
            &[
                Rule::new(1, 1, Slots::Subject(2049), 1),
                Rule::new(1, 1, Slots::Subject(2049), 1),
            ],
            1_000_000,
        )
        .is_err());
        assert!(owner(1, 1, Slots::NoSubject, 1).rule_id(0).is_some());
    }

    #[test]
    fn r05_denials_do_not_slide_or_reset_bucket_debt() {
        let owner = owner(1, 10, Slots::NoSubject, 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&owner, rule, FixedKey::Route, 0),
            RateDecision::Charged
        );
        for (now, retry) in [(1, 9), (2, 8), (8, 2)] {
            assert_eq!(
                charge_at(&owner, rule, FixedKey::Route, now),
                RateDecision::Limited { retry_ns: retry }
            );
            let state = owner.state.lock().expect("unpoisoned test state");
            let bucket = state.rules[0].route.expect("route bucket retained");
            assert_eq!(bucket.tokens, 0);
            assert_eq!(bucket.anchor, 0);
            assert_eq!(bucket.last_charge, 0);
        }
        assert_eq!(
            charge_at(&owner, rule, FixedKey::Route, 10),
            RateDecision::Charged
        );
    }

    #[test]
    fn r09_subject_buckets_are_separated_by_trusted_domain_id() {
        let owner = owner(1, 10, Slots::Subject(2), 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&owner, rule, subject(1, 7), 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(2, 7), 0),
            RateDecision::Charged
        );
        assert_eq!(
            charge_at(&owner, rule, subject(1, 7), 0),
            RateDecision::Limited { retry_ns: 10 }
        );
        assert_eq!(
            charge_at(&owner, rule, subject(2, 7), 0),
            RateDecision::Limited { retry_ns: 10 }
        );
        let state = owner.state.lock().expect("unpoisoned test state");
        let keys = state.rules[0]
            .subjects
            .iter()
            .filter_map(Option::as_ref)
            .map(|entry| entry.key)
            .collect::<Vec<_>>();
        assert!(keys.contains(&(DomainId::from_trusted_config(1), 7)));
        assert!(keys.contains(&(DomainId::from_trusted_config(2), 7)));
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn r12_backward_clock_fault_is_sticky_and_preserves_bucket() {
        let owner = owner(1, 10, Slots::NoSubject, 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            charge_at(&owner, rule, FixedKey::Route, 10),
            RateDecision::Charged
        );
        let before = {
            let state = owner.state.lock().expect("unpoisoned test state");
            let bucket = state.rules[0].route.expect("route bucket retained");
            (
                bucket.tokens,
                bucket.anchor,
                bucket.last_charge,
                state.last_now,
            )
        };
        assert_eq!(
            charge_at(&owner, rule, FixedKey::Route, 9),
            RateDecision::ClockFault
        );
        assert_eq!(
            charge_at(&owner, rule, FixedKey::Route, 20),
            RateDecision::ClockFault
        );
        let state = owner.state.lock().expect("unpoisoned test state");
        let bucket = state.rules[0].route.expect("route bucket retained");
        assert_eq!(
            (
                bucket.tokens,
                bucket.anchor,
                bucket.last_charge,
                state.last_now
            ),
            before
        );
        assert!(state.clock_fault);
    }

    #[test]
    fn r13_elapsed_conversion_failure_is_sticky_without_refill_or_charge() {
        assert_eq!(
            super::checked_elapsed_ns(Duration::from_secs(u64::MAX)),
            None
        );
        let owner = owner(1, 10, Slots::NoSubject, 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            owner.charge_at_with_test_clock(rule, FixedKey::Route, 0),
            RateDecision::Charged
        );
        let before = {
            let state = owner.state.lock().expect("unpoisoned test state");
            let bucket = state.rules[0].route.expect("route bucket retained");
            (
                bucket.tokens,
                bucket.anchor,
                bucket.last_charge,
                state.last_now,
            )
        };
        assert_eq!(
            owner.charge_with_clock(rule, FixedKey::Route, || None),
            RateDecision::ClockFault
        );
        let should_not_sample = Cell::new(false);
        assert_eq!(
            owner.charge_with_clock(rule, FixedKey::Route, || {
                should_not_sample.set(true);
                Some(20)
            }),
            RateDecision::ClockFault
        );
        assert!(!should_not_sample.get());
        let state = owner.state.lock().expect("unpoisoned test state");
        let bucket = state.rules[0].route.expect("route bucket retained");
        assert_eq!(
            (
                bucket.tokens,
                bucket.anchor,
                bucket.last_charge,
                state.last_now
            ),
            before
        );
        assert!(state.clock_fault);
    }

    #[test]
    fn r14_controlled_clock_is_sampled_under_transition_lock() {
        let owner = Arc::new(owner(2, 10, Slots::NoSubject, 1));
        let rule = owner.rule_id(0).expect("fixed rule ID");
        let barrier = Arc::new(Barrier::new(3));
        let next_tick = Arc::new(AtomicU64::new(0));
        let threads = (0..2)
            .map(|_| {
                let owner = Arc::clone(&owner);
                let barrier = Arc::clone(&barrier);
                let next_tick = Arc::clone(&next_tick);
                std::thread::spawn(move || {
                    barrier.wait();
                    owner.charge_with_clock(rule, FixedKey::Route, || {
                        assert!(matches!(
                            owner.state.try_lock(),
                            Err(TryLockError::WouldBlock)
                        ));
                        Some(next_tick.fetch_add(1, Ordering::SeqCst))
                    })
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        let decisions = threads
            .into_iter()
            .map(|thread| thread.join().expect("clock worker completed"))
            .collect::<Vec<_>>();
        assert_eq!(
            decisions
                .iter()
                .filter(|decision| **decision == RateDecision::Charged)
                .count(),
            2
        );
        assert_eq!(next_tick.load(Ordering::SeqCst), 2);
        let state = owner.state.lock().expect("unpoisoned test state");
        assert_eq!(state.last_now, Some(1));
        assert!(!state.clock_fault);
        assert_eq!(
            state.rules[0].route.expect("route bucket retained").tokens,
            0
        );
    }

    #[test]
    fn r15_arc_handles_share_one_owner_but_separate_owner_has_fresh_state() {
        let shared = Arc::new(owner(1, 1_000_000_000_000, Slots::NoSubject, 1));
        let first_handle = Arc::clone(&shared);
        let second_handle = Arc::clone(&shared);
        let rule = shared.rule_id(0).expect("fixed rule ID");
        assert_eq!(
            first_handle.charge(rule, FixedKey::Route),
            RateDecision::Charged
        );
        assert!(matches!(
            second_handle.charge(rule, FixedKey::Route),
            RateDecision::Limited { .. }
        ));
        let separate = owner(1, 1_000_000_000_000, Slots::NoSubject, 1);
        let separate_rule = separate.rule_id(0).expect("separate fixed rule ID");
        assert_eq!(
            separate.charge(separate_rule, FixedKey::Route),
            RateDecision::Charged
        );
    }

    #[test]
    fn r16_close_preserves_debt_and_prevents_sampling_or_new_key() {
        let owner = owner(1, 10, Slots::Subject(1), 1);
        let rule = owner.rule_id(0).expect("fixed rule ID");
        let key = subject(1, 7);
        assert_eq!(
            owner.charge_at_with_test_clock(rule, key, 1),
            RateDecision::Charged
        );
        owner.close();
        let sampled = Cell::new(false);
        assert_eq!(
            owner.charge_with_clock(rule, subject(1, 8), || {
                sampled.set(true);
                Some(100)
            }),
            RateDecision::Closing
        );
        assert!(!sampled.get());
        let state = owner.state.lock().expect("unpoisoned test state");
        let bucket = state.rules[0].subjects[0]
            .expect("original subject retained")
            .bucket;
        assert_eq!(
            (bucket.tokens, bucket.anchor, bucket.last_charge),
            (0, 1, 1)
        );
        assert_eq!(state.used_subject_slots(rule), 1);
        assert_eq!(state.last_now, Some(1));
        assert!(state.closing);
    }

    #[test]
    fn r18_three_simultaneous_same_tick_charges_accept_exact_burst() {
        let owner = Arc::new(owner(2, 10, Slots::NoSubject, 1));
        let rule = owner.rule_id(0).expect("fixed rule ID");
        let barrier = Arc::new(Barrier::new(4));
        let threads = (0..3)
            .map(|_| {
                let owner = Arc::clone(&owner);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    owner.charge_at_with_test_clock(rule, FixedKey::Route, 7)
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        let decisions = threads
            .into_iter()
            .map(|thread| thread.join().expect("charge worker completed"))
            .collect::<Vec<_>>();
        assert_eq!(
            decisions
                .iter()
                .filter(|decision| **decision == RateDecision::Charged)
                .count(),
            2
        );
        assert_eq!(
            decisions
                .iter()
                .filter(|decision| **decision == RateDecision::Limited { retry_ns: 10 })
                .count(),
            1
        );
        let state = owner.state.lock().expect("unpoisoned test state");
        assert_eq!(state.last_now, Some(7));
        assert_eq!(
            state.rules[0].route.expect("route bucket retained").tokens,
            0
        );
    }
}
