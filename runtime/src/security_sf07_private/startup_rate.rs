//! Private checked adapter for the adopted two-stage HTTP rate catalog.
//! No HTTP attachment, subject extraction, public API, default or new limiter.
//! Caller-owned App metadata must be measured by the eventual startup adapter;
//! it is an explicit private input, never silently assumed free.
use super::indexed::{self, DomainId, FixedKey, RateDecision, RateOwner, Rule, RuleId, Slots};
use std::{mem::size_of, time::Duration, time::Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Error {
    Invalid,
    Allocation,
    Fault,
}
impl From<indexed::StartupError> for Error {
    fn from(error: indexed::StartupError) -> Self {
        match error {
            indexed::StartupError::Invalid => Self::Invalid,
            indexed::StartupError::Allocation => Self::Allocation,
            indexed::StartupError::Internal => Self::Fault,
        }
    }
}
fn positive(value: i64) -> Result<usize, Error> {
    usize::try_from(value)
        .ok()
        .filter(|value| *value > 0 && *value <= isize::MAX as usize)
        .ok_or(Error::Invalid)
}
fn add(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b)
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or(Error::Invalid)
}
fn array<T>(count: usize) -> Result<usize, Error> {
    count
        .checked_mul(size_of::<T>())
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or(Error::Invalid)
}
fn millis(value: i64) -> Result<u64, Error> {
    let value = u64::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or(Error::Invalid)?;
    Instant::now()
        .checked_add(Duration::from_millis(value))
        .ok_or(Error::Invalid)?;
    value.checked_mul(1_000_000).ok_or(Error::Invalid)
}

// These become opaque façade values at the public cutover. No Clone/Copy,
// serialization, zero-as-unlimited interpretation or implicit defaults.
pub(crate) struct Capacity {
    rules: usize,
    domains: usize,
    subject_slots: usize,
    retained_bytes: usize,
    request_work: u64,
}
impl Capacity {
    // Caller temporary Record storage is checked before that Vec allocates.
    pub(crate) fn admit_catalogue(&self, records: usize, retained: usize) -> Result<(), Error> {
        if records > self.rules || retained > self.retained_bytes {
            Err(Error::Invalid)
        } else {
            Ok(())
        }
    }

    pub(crate) fn checked(
        rules: i64,
        domains: i64,
        subject_slots: i64,
        retained_bytes: i64,
        request_work: i64,
    ) -> Result<Self, Error> {
        let domains = positive(domains)?;
        if domains > usize::from(u16::MAX) + 1 {
            return Err(Error::Invalid);
        }
        Ok(Self {
            rules: positive(rules)?,
            domains,
            subject_slots: positive(subject_slots)?,
            retained_bytes: positive(retained_bytes)?,
            request_work: u64::try_from(request_work)
                .ok()
                .filter(|n| *n > 0)
                .ok_or(Error::Invalid)?,
        })
    }
}
pub(crate) struct Policy {
    route: Rule,
    subject: Option<Rule>,
}
impl Policy {
    pub(crate) fn has_subject(&self) -> bool {
        self.subject.is_some()
    }
    pub(crate) fn record(&self, subject_support: SubjectSupport) -> Record {
        // Copy checked immutable scalars only, never limiter state/brands.
        Record {
            policy: Self {
                route: self.route,
                subject: self.subject,
            },
            subject_support,
        }
    }

    pub(crate) fn checked(burst: i64, refill_ms: i64) -> Result<Self, Error> {
        let burst = u32::try_from(burst)
            .ok()
            .filter(|n| *n > 0)
            .ok_or(Error::Invalid)?;
        let refill = millis(refill_ms)?;
        // NoSubject has no idle table. Derive its required scalar from the
        // explicit refill; this is not a hidden operational default.
        let route = Rule::new(burst, refill, Slots::NoSubject, refill);
        route.checked_layout()?;
        Ok(Self {
            route,
            subject: None,
        })
    }
    pub(crate) fn with_subject(
        mut self,
        burst: i64,
        refill_ms: i64,
        subject_slots: i64,
        idle_ms: i64,
    ) -> Result<Self, Error> {
        if self.subject.is_some() {
            return Err(Error::Invalid);
        }
        let burst = u32::try_from(burst)
            .ok()
            .filter(|n| *n > 0)
            .ok_or(Error::Invalid)?;
        let subject = Rule::new(
            burst,
            millis(refill_ms)?,
            Slots::Subject(positive(subject_slots)?),
            millis(idle_ms)?,
        );
        subject.checked_layout()?;
        self.subject = Some(subject);
        Ok(self)
    }
}
// The public route wrapper must derive this from its sealed preparation profile,
// never from an arbitrary handler argument, i64, header or unverified claims.
pub(crate) enum SubjectSupport {
    None,
    Verified,
}
pub(crate) struct Record {
    pub(crate) policy: Policy,
    pub(crate) subject_support: SubjectSupport,
}
#[derive(Clone, Copy)]
pub(crate) struct Binding {
    route: RuleId,
    subject: Option<(RuleId, DomainId)>,
}
impl Binding {
    pub(crate) fn has_subject(self) -> bool {
        self.subject.is_some()
    }
}
pub(crate) struct Plan {
    owner: RateOwner,
    bindings: Vec<Binding>,
    retained_bytes: usize,
    #[cfg(test)]
    request_work: u64,
}
fn bridge_bytes(records: usize, rules: usize, catalog_bytes: usize) -> Result<usize, Error> {
    // Include the temporary Rule vector conservatively even after it drops.
    // Retained plan/Arc metadata and BOTH vector capacities are checked again
    // after fallible reservations. Native allocator overhead/RSS is separate.
    add(
        add(size_of::<Plan>(), 2 * size_of::<usize>())?,
        add(
            add(array::<Binding>(records)?, array::<Rule>(rules)?)?,
            catalog_bytes,
        )?,
    )
}
impl Plan {
    #[cfg(test)]
    pub(crate) fn checked(
        records: &[Record],
        limits: Capacity,
        catalog_bytes: usize,
    ) -> Result<Option<Self>, Error> {
        Self::checked_ref(records, &limits, catalog_bytes)
    }
    pub(crate) fn checked_ref(
        records: &[Record],
        limits: &Capacity,
        catalog_bytes: usize,
    ) -> Result<Option<Self>, Error> {
        if records.is_empty() {
            return Ok(None);
        }
        if records.len() > limits.rules {
            return Err(Error::Invalid);
        }
        let mut rules = records.len();
        let mut domains = 0usize;
        let mut slots = 0usize;
        let mut additional = 0usize;
        let mut request_work = 0u64;
        for record in records {
            let route = record.policy.route.checked_layout()?;
            let mut work = route.work;
            if let Some(subject) = record.policy.subject {
                if !matches!(record.subject_support, SubjectSupport::Verified) {
                    return Err(Error::Invalid);
                }
                let descriptor = subject.checked_layout()?;
                rules = add(rules, 1)?;
                domains = add(domains, 1)?;
                slots = add(slots, descriptor.subject_slots)?;
                additional = add(additional, descriptor.additional_bytes)?;
                work = work.checked_add(descriptor.work).ok_or(Error::Invalid)?;
            }
            request_work = request_work.max(work);
        }
        if rules > limits.rules
            || domains > limits.domains
            || slots > limits.subject_slots
            || request_work > limits.request_work
        {
            return Err(Error::Invalid);
        }
        let count = add(rules, slots)?;
        let minimum_core = add(RateOwner::minimum_fixed_bytes(rules)?, additional)?;
        let minimum_bridge = bridge_bytes(records.len(), rules, catalog_bytes)?;
        if add(minimum_core, minimum_bridge)? > limits.retained_bytes {
            return Err(Error::Invalid);
        }
        let mut native = Vec::new();
        native
            .try_reserve_exact(rules)
            .map_err(|_| Error::Allocation)?;
        let mut bindings = Vec::new();
        bindings
            .try_reserve_exact(records.len())
            .map_err(|_| Error::Allocation)?;
        let bridge = bridge_bytes(bindings.capacity(), native.capacity(), catalog_bytes)?;
        let core_budget = limits
            .retained_bytes
            .checked_sub(bridge)
            .ok_or(Error::Invalid)?;
        if minimum_core > core_budget {
            return Err(Error::Invalid);
        }
        for record in records {
            native.push(record.policy.route);
            if let Some(subject) = record.policy.subject {
                native.push(subject);
            }
        }
        let owner = RateOwner::checked_rate(
            &native,
            indexed::Limits {
                count,
                bytes: core_budget,
                work: limits.request_work,
            },
        )?;
        if owner.work_bound() > request_work {
            return Err(Error::Fault);
        }
        let mut next = 0usize;
        let mut domain = 0usize;
        for record in records {
            let route = owner.rule_id(next).ok_or(Error::Fault)?;
            next = add(next, 1)?;
            let subject = if record.policy.subject.is_some() {
                let rule = owner.rule_id(next).ok_or(Error::Fault)?;
                next = add(next, 1)?;
                let id = DomainId::from_trusted_config(
                    u16::try_from(domain).map_err(|_| Error::Invalid)?,
                );
                domain = add(domain, 1)?;
                Some((rule, id))
            } else {
                None
            };
            bindings.push(Binding { route, subject });
        }
        let retained_bytes = add(bridge, owner.retained_bytes())?;
        if retained_bytes > limits.retained_bytes {
            return Err(Error::Invalid);
        }
        Ok(Some(Self {
            owner,
            bindings,
            retained_bytes,
            #[cfg(test)]
            request_work,
        }))
    }
    pub(crate) fn binding(&self, record: usize) -> Option<Binding> {
        self.bindings.get(record).copied()
    }
    pub(crate) fn charge_route(&self, binding: Binding) -> RateDecision {
        self.owner.charge(binding.route, FixedKey::Route)
    }
    pub(crate) fn charge_subject(&self, binding: Binding, subject: i64) -> RateDecision {
        let Some((rule, domain)) = binding.subject else {
            return RateDecision::Fault;
        };
        self.owner.charge(rule, FixedKey::subject(domain, subject))
    }
    pub(crate) fn close(&self) {
        self.owner.close();
    }
    pub(crate) fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    #[cfg(test)]
    pub(crate) fn request_work(&self) -> u64 {
        self.request_work
    }
}

#[cfg(test)]
#[path = "startup_rate_tests.rs"]
mod tests;
