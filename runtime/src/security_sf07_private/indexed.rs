//! Private finite indexed rate core. No public numbers/API or HTTP integration.
use std::mem::size_of;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const KEY_BITS: usize = 80;
const MAX_EDITS: usize = usize::BITS as usize + 16;
const CLEANUP_WORK: u64 = MAX_EDITS as u64 + 2;
static NEXT_BRAND: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuleId {
    brand: u64,
    index: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DomainId(u16);
impl DomainId {
    pub(crate) const fn from_trusted_config(v: u16) -> Self {
        Self(v)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FixedKey {
    Route,
    Subject { domain: DomainId, subject: i64 },
}
impl FixedKey {
    pub(crate) const fn subject(domain: DomainId, subject: i64) -> Self {
        Self::Subject { domain, subject }
    }
}
type Key = (DomainId, i64);
type LookupPath = (Link, Option<(usize, usize)>, Option<(usize, usize)>);
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
#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    pub count: usize,
    pub bytes: usize,
    pub work: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StartupError {
    Invalid,
    Allocation,
    Internal,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RateDecision {
    Charged,
    Limited { retry_ns: u64 },
    Capacity,
    Closing,
    ClockFault,
    Fault,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bucket {
    tokens: u32,
    anchor: u64,
    last_charge: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    key: Key,
    bucket: Bucket,
    due: u128,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Link {
    Empty,
    Leaf(usize),
    Branch(usize),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Branch {
    active: bool,
    bit: usize,
    child: [Link; 2],
    next: Option<usize>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Summary {
    free: bool,
    due: Option<u128>,
}
fn merge(a: Summary, b: Summary) -> Summary {
    Summary {
        free: a.free || b.free,
        due: match (a.due, b.due) {
            (Some(x), Some(y)) => Some(x.min(y)),
            (Some(x), None) | (None, Some(x)) => Some(x),
            _ => None,
        },
    }
}
struct Subjects {
    entries: Vec<Option<Entry>>,
    branches: Vec<Branch>,
    summary: Vec<Summary>,
    p: usize,
    root: Link,
    free_branch: Option<usize>,
    faulted: bool,
}
struct RuleState {
    config: Rule,
    route: Option<Bucket>,
    subjects: Subjects,
}
struct RateState {
    rules: Vec<RuleState>,
    brand: u64,
    last_now: Option<u64>,
    faulted: bool,
    clock_fault: bool,
    closing: bool,
    work_cap: u64,
    last_work: u64,
}
pub(crate) struct RateOwner {
    state: Mutex<RateState>,
    origin: Instant,
    brand: u64,
    rule_count: usize,
    retained_bytes: usize,
    work_bound: u64,
}

// All work units are bounded semantic blocks, not machine instructions/time.
// Every helper loop iteration/node/key read/journal write is charged. One unit
// covers a constant-size primitive block (<=32 scalar operations); see WORK.md.
fn work_bound(height: usize) -> Option<u64> {
    27u64
        .checked_mul(KEY_BITS as u64)?
        .checked_add(21u64.checked_mul(u64::try_from(height).ok()?)?)?
        .checked_add(MAX_EDITS as u64)?
        .checked_add(256)?
        .checked_add(CLEANUP_WORK)
}

fn layout(n: usize) -> Option<(usize, usize, usize)> {
    if n == 0 {
        return None;
    }
    let p = n.checked_next_power_of_two()?;
    Some((p, p.checked_mul(2)?, p.trailing_zeros() as usize))
}
fn array_bytes<T>(count: usize) -> Option<usize> {
    let bytes = count.checked_mul(size_of::<T>())?;
    (bytes <= isize::MAX as usize).then_some(bytes)
}
fn vec_bytes<T>(v: &Vec<T>) -> Option<usize> {
    array_bytes::<T>(v.capacity())
}
fn fresh_brand(counter: &AtomicU64) -> Result<u64, StartupError> {
    // Startup-only monotone CAS; never wraps/reuses. No request-path retry loop.
    let mut current = counter.load(Ordering::Relaxed);
    loop {
        let next = current
            .checked_add(1)
            .filter(|_| current != 0)
            .ok_or(StartupError::Invalid)?;
        match counter.compare_exchange(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return Ok(current),
            Err(observed) => current = observed,
        }
    }
}

fn vector<T: Clone>(n: usize, value: T) -> Result<Vec<T>, StartupError> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| StartupError::Allocation)?;
    v.resize(n, value);
    Ok(v)
}
impl Subjects {
    fn empty() -> Self {
        Self {
            entries: Vec::new(),
            branches: Vec::new(),
            summary: Vec::new(),
            p: 0,
            root: Link::Empty,
            free_branch: None,
            faulted: false,
        }
    }
    fn checked(n: usize) -> Result<Self, StartupError> {
        let (p, total, _) = layout(n).ok_or(StartupError::Invalid)?;
        let entries = vector(n, None)?;
        let mut branches = vector(
            n - 1,
            Branch {
                active: false,
                bit: 0,
                child: [Link::Empty; 2],
                next: None,
            },
        )?;
        for (i, b) in branches.iter_mut().enumerate() {
            b.next = (i + 1 < n - 1).then_some(i + 1);
        }
        let mut summary = vector(total, Summary::default())?;
        for item in &mut summary[p..p + n] {
            item.free = true;
        }
        for i in (1..p).rev() {
            summary[i] = merge(summary[2 * i], summary[2 * i + 1]);
        }
        Ok(Self {
            entries,
            branches,
            summary,
            p,
            root: Link::Empty,
            free_branch: (n > 1).then_some(0),
            faulted: false,
        })
    }
}
impl RateOwner {
    pub(crate) fn checked_rate(rules: &[Rule], limits: Limits) -> Result<Self, StartupError> {
        Self::checked_with(rules, limits, |_| {})
    }
    // Unit-only callers use a capacity-padding callback; production has no callback.
    fn checked_with<F: FnOnce(&mut Vec<RuleState>)>(
        rules: &[Rule],
        limits: Limits,
        pad: F,
    ) -> Result<Self, StartupError> {
        if rules.is_empty() || limits.count == 0 || limits.bytes == 0 || limits.work == 0 {
            return Err(StartupError::Invalid);
        }
        let mut count = rules.len();
        let mut bytes = size_of::<Self>()
            .checked_add(size_of::<AtomicU64>())
            .ok_or(StartupError::Invalid)?;
        bytes = bytes
            .checked_add(array_bytes::<RuleState>(rules.len()).ok_or(StartupError::Invalid)?)
            .ok_or(StartupError::Invalid)?;
        let mut max_work = 48u64
            .checked_add(CLEANUP_WORK)
            .ok_or(StartupError::Invalid)?;
        for r in rules {
            if r.burst == 0 || r.refill_ns == 0 || r.idle_ns == 0 {
                return Err(StartupError::Invalid);
            }
            if let Slots::Subject(n) = r.subject_slots {
                count = count.checked_add(n).ok_or(StartupError::Invalid)?;
                let (_, total, h) = layout(n).ok_or(StartupError::Invalid)?;
                bytes = bytes
                    .checked_add(array_bytes::<Option<Entry>>(n).ok_or(StartupError::Invalid)?)
                    .and_then(|v| v.checked_add(array_bytes::<Branch>(n - 1)?))
                    .and_then(|v| v.checked_add(array_bytes::<Summary>(total)?))
                    .ok_or(StartupError::Invalid)?;
                max_work = max_work.max(work_bound(h).ok_or(StartupError::Invalid)?);
            }
        }
        if count > limits.count || bytes > limits.bytes || limits.work < max_work {
            return Err(StartupError::Invalid);
        }
        let mut states = Vec::new();
        states
            .try_reserve_exact(rules.len())
            .map_err(|_| StartupError::Allocation)?;
        for r in rules {
            let subjects = match r.subject_slots {
                Slots::NoSubject => Subjects::empty(),
                Slots::Subject(n) => Subjects::checked(n)?,
            };
            states.push(RuleState {
                config: *r,
                route: None,
                subjects,
            });
        }
        pad(&mut states);
        let actual = size_of::<Self>()
            .checked_add(size_of::<AtomicU64>())
            .and_then(|v| v.checked_add(vec_bytes(&states)?))
            .and_then(|v| {
                states.iter().try_fold(v, |sum, r| {
                    sum.checked_add(vec_bytes(&r.subjects.entries)?)
                        .and_then(|v| v.checked_add(vec_bytes(&r.subjects.branches)?))
                        .and_then(|v| v.checked_add(vec_bytes(&r.subjects.summary)?))
                })
            })
            .ok_or(StartupError::Invalid)?;
        if actual > limits.bytes {
            return Err(StartupError::Invalid);
        }
        let brand = fresh_brand(&NEXT_BRAND)?;
        let state = Mutex::new(RateState {
            rules: states,
            brand,
            last_now: None,
            faulted: false,
            clock_fault: false,
            closing: false,
            work_cap: limits.work,
            last_work: 0,
        });
        // Initialize the platform mutex before a request can be measured.
        match state.lock() {
            Ok(guard) => drop(guard),
            Err(_poisoned) => return Err(StartupError::Internal),
        }
        Ok(Self {
            state,
            origin: Instant::now(),
            brand,
            rule_count: rules.len(),
            retained_bytes: actual,
            work_bound: max_work,
        })
    }
    pub(crate) fn rule_id(&self, index: usize) -> Option<RuleId> {
        (index < self.rule_count).then_some(RuleId {
            brand: self.brand,
            index,
        })
    }
    pub(crate) fn charge(&self, rule: RuleId, key: FixedKey) -> RateDecision {
        self.charge_with_clock(rule, key, || {
            Instant::now()
                .checked_duration_since(self.origin)
                .and_then(checked_elapsed_ns)
        })
    }
    fn charge_with_clock<F: FnOnce() -> Option<u64>>(
        &self,
        rule: RuleId,
        key: FixedKey,
        sample: F,
    ) -> RateDecision {
        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(p) => {
                p.into_inner().faulted = true;
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
        // A foreign ID must not sample the receiver's clock or touch debt.
        if rule.brand != state.brand || rule.index >= state.rules.len() {
            state.faulted = true;
            return RateDecision::Fault;
        }
        let Some(now) = sample() else {
            state.clock_fault = true;
            return RateDecision::ClockFault;
        };
        state.charge_at(rule, key, now)
    }
    pub(crate) fn close(&self) {
        match self.state.lock() {
            Ok(mut s) => s.closing = true,
            Err(p) => p.into_inner().faulted = true,
        }
    }
    #[cfg(test)]
    fn charge_at_with_test_clock(&self, rule: RuleId, key: FixedKey, now: u64) -> RateDecision {
        self.charge_with_clock(rule, key, || Some(now))
    }
}
fn checked_elapsed_ns(elapsed: Duration) -> Option<u64> {
    u64::try_from(elapsed.as_nanos()).ok()
}
#[derive(Clone, Copy, Debug)]
enum Failure {
    Clock,
    Fault,
}
struct Work {
    used: u64,
    cap: u64,
    cleanup: u64,
    #[cfg(test)]
    panic_after: Option<u64>,
}
impl Work {
    fn new(cap: u64) -> Result<Self, Failure> {
        Ok(Self {
            used: 0,
            cap: cap.checked_sub(CLEANUP_WORK).ok_or(Failure::Fault)?,
            cleanup: 0,
            #[cfg(test)]
            panic_after: None,
        })
    }
    fn tick(&mut self, n: u64) -> Result<(), Failure> {
        let next = self.used.checked_add(n).ok_or(Failure::Fault)?;
        if next > self.cap {
            return Err(Failure::Fault);
        }
        self.used = next;
        #[cfg(test)]
        if self.panic_after == Some(next) {
            panic!("private mutation fault oracle");
        }
        Ok(())
    }
}
impl RateState {
    fn charge_at(&mut self, id: RuleId, key: FixedKey, now: u64) -> RateDecision {
        if self.faulted {
            return RateDecision::Fault;
        }
        if self.clock_fault {
            return RateDecision::ClockFault;
        }
        if self.closing {
            return RateDecision::Closing;
        }
        if id.brand != self.brand || id.index >= self.rules.len() {
            self.faulted = true;
            return RateDecision::Fault;
        }
        if self.last_now.is_some_and(|last| now < last) {
            self.clock_fault = true;
            return RateDecision::ClockFault;
        }
        self.last_now = Some(now);
        let mut work = match Work::new(self.work_cap) {
            Ok(w) => w,
            Err(_) => {
                self.faulted = true;
                return RateDecision::Fault;
            }
        };
        let result = work
            .tick(32)
            .and_then(|()| charge_rule(&mut self.rules[id.index], key, now, &mut work));
        self.last_work = match work.used.checked_add(work.cleanup) {
            Some(v) => v,
            None => {
                self.faulted = true;
                return RateDecision::Fault;
            }
        };
        match result {
            Ok(v) => v,
            Err(Failure::Clock) => {
                self.clock_fault = true;
                RateDecision::ClockFault
            }
            Err(Failure::Fault) => {
                self.faulted = true;
                RateDecision::Fault
            }
        }
    }
}
fn refill(b: &mut Bucket, burst: u32, refill_ns: u64, now: u64) -> Result<(), Failure> {
    if b.tokens > burst || refill_ns == 0 {
        return Err(Failure::Fault);
    }
    let elapsed = now.checked_sub(b.anchor).ok_or(Failure::Clock)?;
    let periods = elapsed / refill_ns;
    if periods != 0 {
        let missing = u64::from(burst - b.tokens);
        b.tokens = if periods >= missing {
            burst
        } else {
            b.tokens + periods as u32
        };
        b.anchor = now.checked_sub(elapsed % refill_ns).ok_or(Failure::Fault)?;
    }
    Ok(())
}
fn charge_bucket(b: &mut Bucket, r: Rule, now: u64) -> Result<RateDecision, Failure> {
    refill(b, r.burst, r.refill_ns, now)?;
    if b.tokens == 0 {
        let retry_ns = r
            .refill_ns
            .checked_sub(now.checked_sub(b.anchor).ok_or(Failure::Clock)? % r.refill_ns)
            .filter(|n| *n > 0)
            .ok_or(Failure::Fault)?;
        return Ok(RateDecision::Limited { retry_ns });
    }
    b.tokens -= 1;
    b.last_charge = now;
    Ok(RateDecision::Charged)
}
fn due(b: Bucket, r: Rule) -> Result<u128, Failure> {
    let missing = r.burst.checked_sub(b.tokens).ok_or(Failure::Fault)?;
    let full = u128::from(b.anchor)
        .checked_add(
            u128::from(missing)
                .checked_mul(u128::from(r.refill_ns))
                .ok_or(Failure::Fault)?,
        )
        .ok_or(Failure::Fault)?;
    Ok(full.max(
        u128::from(b.last_charge)
            .checked_add(u128::from(r.idle_ns))
            .ok_or(Failure::Fault)?,
    ))
}
#[derive(Clone, Copy)]
enum Undo {
    Empty,
    Branch(usize, Branch),
    Entry(usize, Option<Entry>),
    Summary(usize, Summary),
}
// Rollback is infallible/bounded: arrays never resize, every recorded index was
// checked before mutation. Drop never unwraps/allocates/panics on unwind.
struct Tx<'a> {
    state: &'a mut Subjects,
    work: &'a mut Work,
    root: Link,
    free: Option<usize>,
    undo: [Undo; MAX_EDITS],
    used: usize,
    committed: bool,
}
impl<'a> Tx<'a> {
    fn new(state: &'a mut Subjects, work: &'a mut Work) -> Result<Self, Failure> {
        work.tick(MAX_EDITS as u64)?;
        Ok(Self {
            root: state.root,
            free: state.free_branch,
            state,
            work,
            undo: [Undo::Empty; MAX_EDITS],
            used: 0,
            committed: false,
        })
    }
    fn log(&mut self, u: Undo) -> Result<(), Failure> {
        self.work.tick(2)?;
        let dst = self.undo.get_mut(self.used).ok_or(Failure::Fault)?;
        *dst = u;
        self.used += 1;
        Ok(())
    }
    fn branch(&mut self, i: usize) -> Result<Branch, Failure> {
        self.work.tick(3)?;
        let b = *self.state.branches.get(i).ok_or(Failure::Fault)?;
        if !b.active || b.bit >= KEY_BITS || b.child.contains(&Link::Empty) {
            return Err(Failure::Fault);
        }
        Ok(b)
    }
    fn entry(&mut self, i: usize) -> Result<Entry, Failure> {
        self.work.tick(2)?;
        self.state
            .entries
            .get(i)
            .copied()
            .flatten()
            .ok_or(Failure::Fault)
    }
    fn write_branch(&mut self, i: usize, b: Branch) -> Result<(), Failure> {
        let old = *self.state.branches.get(i).ok_or(Failure::Fault)?;
        self.log(Undo::Branch(i, old))?;
        self.state.branches[i] = b;
        self.work.tick(1)
    }
    fn write_entry(&mut self, i: usize, e: Entry) -> Result<(), Failure> {
        let old = *self.state.entries.get(i).ok_or(Failure::Fault)?;
        self.log(Undo::Entry(i, old))?;
        self.state.entries[i] = Some(e);
        self.work.tick(1)
    }
    fn summary(&mut self, i: usize) -> Result<Summary, Failure> {
        self.work.tick(1)?;
        self.state.summary.get(i).copied().ok_or(Failure::Fault)
    }
    fn check_summary(&mut self, i: usize) -> Result<Summary, Failure> {
        let s = self.summary(i)?;
        let expected = if i < self.state.p {
            merge(self.summary(i * 2)?, self.summary(i * 2 + 1)?)
        } else {
            let leaf = i.checked_sub(self.state.p).ok_or(Failure::Fault)?;
            if leaf >= self.state.entries.len() {
                Summary::default()
            } else {
                self.work.tick(1)?;
                match self.state.entries[leaf] {
                    None => Summary {
                        free: true,
                        due: None,
                    },
                    Some(e) => Summary {
                        free: false,
                        due: Some(e.due),
                    },
                }
            }
        };
        if s != expected {
            return Err(Failure::Fault);
        }
        Ok(s)
    }
    fn select(&mut self, now: u64) -> Result<Option<usize>, Failure> {
        let root = self.check_summary(1)?;
        let free = root.free;
        if !free && root.due.is_none_or(|v| v > u128::from(now)) {
            return Ok(None);
        }
        let mut i = 1;
        while i < self.state.p {
            self.work.tick(4)?;
            self.check_summary(i)?;
            let left = self.check_summary(2 * i)?;
            let right = self.check_summary(2 * i + 1)?;
            let usable = |s: Summary| {
                if free {
                    s.free
                } else {
                    s.due.is_some_and(|v| v <= u128::from(now))
                }
            };
            i = if usable(left) {
                i * 2
            } else if usable(right) {
                i * 2 + 1
            } else {
                return Err(Failure::Fault);
            };
        }
        self.check_summary(i)?;
        let slot = i - self.state.p;
        if slot >= self.state.entries.len() {
            return Err(Failure::Fault);
        }
        Ok(Some(slot))
    }
    fn update_summary(&mut self, slot: usize) -> Result<(), Failure> {
        let e = self.entry(slot)?;
        let mut i = self.state.p.checked_add(slot).ok_or(Failure::Fault)?;
        let mut next = Summary {
            free: false,
            due: Some(e.due),
        };
        loop {
            self.work.tick(3)?;
            let old = self.summary(i)?;
            self.log(Undo::Summary(i, old))?;
            self.state.summary[i] = next;
            if i == 1 {
                break;
            }
            i /= 2;
            next = merge(self.summary(2 * i)?, self.summary(2 * i + 1)?);
        }
        Ok(())
    }
    fn bit(&mut self, k: Key, bit: usize) -> Result<usize, Failure> {
        self.work.tick(1)?;
        if bit >= KEY_BITS {
            return Err(Failure::Fault);
        }
        Ok(if bit < 16 {
            usize::from((k.0 .0 >> (15 - bit)) & 1)
        } else {
            ((k.1 as u64 >> (79 - bit)) & 1) as usize
        })
    }
    fn lookup(&mut self, key: Key) -> Result<LookupPath, Failure> {
        let mut node = self.state.root;
        let mut parent = None;
        let mut grand = None;
        let mut last = None;
        loop {
            self.work.tick(2)?;
            match node {
                Link::Empty => {
                    if parent.is_some() {
                        return Err(Failure::Fault);
                    }
                    return Ok((node, parent, grand));
                }
                Link::Leaf(i) => {
                    self.entry(i)?;
                    return Ok((node, parent, grand));
                }
                Link::Branch(i) => {
                    let b = self.branch(i)?;
                    if last.is_some_and(|n| b.bit <= n) {
                        return Err(Failure::Fault);
                    }
                    last = Some(b.bit);
                    let side = self.bit(key, b.bit)?;
                    grand = parent;
                    parent = Some((i, side));
                    node = b.child[side];
                }
            }
        }
    }
    fn set_link(
        &mut self,
        parent: Option<(usize, usize)>,
        expected: Link,
        value: Link,
    ) -> Result<(), Failure> {
        self.work.tick(3)?;
        if let Some((i, side)) = parent {
            let mut b = self.branch(i)?;
            if side > 1 || b.child[side] != expected {
                return Err(Failure::Fault);
            }
            b.child[side] = value;
            self.write_branch(i, b)
        } else {
            if self.state.root != expected {
                return Err(Failure::Fault);
            }
            self.state.root = value;
            self.work.tick(1)
        }
    }
    fn delete(&mut self, slot: usize) -> Result<(), Failure> {
        let e = self.entry(slot)?;
        let (found, parent, grand) = self.lookup(e.key)?;
        if found != Link::Leaf(slot) {
            return Err(Failure::Fault);
        }
        if let Some((i, side)) = parent {
            let b = self.branch(i)?;
            let other = b.child[1 - side];
            match other {
                Link::Leaf(j) => {
                    self.entry(j)?;
                }
                Link::Branch(j) => {
                    if self.branch(j)?.bit <= b.bit {
                        return Err(Failure::Fault);
                    }
                }
                Link::Empty => return Err(Failure::Fault),
            }
            self.set_link(grand, Link::Branch(i), other)?;
            self.write_branch(
                i,
                Branch {
                    active: false,
                    bit: 0,
                    child: [Link::Empty; 2],
                    next: self.state.free_branch,
                },
            )?;
            self.state.free_branch = Some(i);
            self.work.tick(1)?;
        } else {
            self.set_link(None, Link::Leaf(slot), Link::Empty)?;
        }
        Ok(())
    }
    fn insert(&mut self, slot: usize, key: Key) -> Result<(), Failure> {
        let (leaf, _, _) = self.lookup(key)?;
        let Link::Leaf(other) = leaf else {
            if leaf != Link::Empty {
                return Err(Failure::Fault);
            }
            return self.set_link(None, Link::Empty, Link::Leaf(slot));
        };
        let old = self.entry(other)?.key;
        let mut different = None;
        for bit in 0..KEY_BITS {
            self.work.tick(1)?;
            if self.bit(key, bit)? != self.bit(old, bit)? {
                different = Some(bit);
                break;
            }
        }
        let bit = different.ok_or(Failure::Fault)?;
        let mut node = self.state.root;
        let mut parent = None;
        let mut last = None;
        while let Link::Branch(i) = node {
            self.work.tick(2)?;
            let b = self.branch(i)?;
            if last.is_some_and(|n| b.bit <= n) || b.bit == bit {
                return Err(Failure::Fault);
            }
            if b.bit > bit {
                break;
            }
            last = Some(b.bit);
            let side = self.bit(key, b.bit)?;
            parent = Some((i, side));
            node = b.child[side];
        }
        let i = self.state.free_branch.ok_or(Failure::Fault)?;
        self.work.tick(4)?;
        let free = *self.state.branches.get(i).ok_or(Failure::Fault)?;
        if free.active || free.next == Some(i) {
            return Err(Failure::Fault);
        }
        if let Some(next) = free.next {
            if self.state.branches.get(next).is_none_or(|b| b.active) {
                return Err(Failure::Fault);
            }
        }
        let side = self.bit(key, bit)?;
        let mut child = [node; 2];
        child[side] = Link::Leaf(slot);
        self.write_branch(
            i,
            Branch {
                active: true,
                bit,
                child,
                next: None,
            },
        )?;
        self.state.free_branch = free.next;
        self.work.tick(1)?;
        self.set_link(parent, node, Link::Branch(i))
    }
    fn commit(mut self) {
        self.committed = true;
    }
}
impl Drop for Tx<'_> {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        self.state.faulted = true;
        self.state.root = self.root;
        self.state.free_branch = self.free;
        for u in self.undo[..self.used].iter().rev() {
            match *u {
                Undo::Branch(i, b) => {
                    if let Some(dst) = self.state.branches.get_mut(i) {
                        *dst = b;
                    }
                }
                Undo::Entry(i, e) => {
                    if let Some(dst) = self.state.entries.get_mut(i) {
                        *dst = e;
                    }
                }
                Undo::Summary(i, s) => {
                    if let Some(dst) = self.state.summary.get_mut(i) {
                        *dst = s;
                    }
                }
                Undo::Empty => {}
            }
        }
        self.work.cleanup += self.used as u64 + 2;
    }
}
fn charge_rule(
    rule: &mut RuleState,
    key: FixedKey,
    now: u64,
    work: &mut Work,
) -> Result<RateDecision, Failure> {
    work.tick(16)?;
    let r = rule.config;
    match (r.subject_slots, key) {
        (Slots::NoSubject, FixedKey::Route) => {
            let mut b = rule.route.unwrap_or(Bucket {
                tokens: r.burst,
                anchor: now,
                last_charge: now,
            });
            let answer = charge_bucket(&mut b, r, now)?;
            rule.route = Some(b);
            Ok(answer)
        }
        (Slots::Subject(_), FixedKey::Subject { domain, subject }) => {
            if rule.subjects.faulted {
                return Err(Failure::Fault);
            }
            let key = (domain, subject);
            let mut tx = Tx::new(&mut rule.subjects, work)?;
            let (found, _, _) = tx.lookup(key)?;
            if let Link::Leaf(i) = found {
                let mut e = tx.entry(i)?;
                tx.work.tick(4)?;
                if e.key == key {
                    let answer = charge_bucket(&mut e.bucket, r, now)?;
                    if answer == RateDecision::Charged {
                        e.due = due(e.bucket, r)?;
                        tx.write_entry(i, e)?;
                        tx.update_summary(i)?;
                    }
                    tx.commit();
                    return Ok(answer);
                }
            }
            let Some(i) = tx.select(now)? else {
                tx.commit();
                return Ok(RateDecision::Capacity);
            };
            tx.work.tick(4)?;
            if let Some(old) = tx.state.entries[i] {
                let mut b = old.bucket;
                refill(&mut b, r.burst, r.refill_ns, now)?;
                if b.tokens != r.burst
                    || now.checked_sub(b.last_charge).ok_or(Failure::Clock)? < r.idle_ns
                    || old.due > u128::from(now)
                {
                    return Err(Failure::Fault);
                }
                tx.delete(i)?;
            }
            let mut b = Bucket {
                tokens: r.burst,
                anchor: now,
                last_charge: now,
            };
            let answer = charge_bucket(&mut b, r, now)?;
            tx.write_entry(
                i,
                Entry {
                    key,
                    bucket: b,
                    due: due(b, r)?,
                },
            )?;
            tx.insert(i, key)?;
            tx.update_summary(i)?;
            tx.commit();
            Ok(answer)
        }
        _ => Err(Failure::Fault),
    }
}

#[cfg(test)]
#[path = "indexed/tests.rs"]
mod tests;
