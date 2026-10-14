//! Private Q1 plan: finite sequential dial cursor and DNS validity ticket.
//! The caller must finish native disposal/actual child join before reporting an attempt failure.

use super::{
    check_candidates, AddressError, CheckedSnapshot, InternalPolicy, ServiceIdentity,
    MAX_CANDIDATES,
};
use std::{
    io::ErrorKind,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    num::NonZeroU64,
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResolverFailure {
    Timeout,
    ServFail,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreferredFamily {
    V4,
    V6,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AttemptOrder {
    Interleave { preferred: PreferredFamily },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Q1Error {
    Address(AddressError),
    FamilyFailure,
    EmptyTtlChain,
    MissingPositiveExpiry,
    InvalidAttemptLimit,
    InvalidConnectBudget,
    DeadlineArithmetic,
    NotConnected,
    EnqueueAlreadyIssued,
    PlanFinished,
    PlanStopped(PlanStop),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CacheDisposition {
    PreserveExistingPositiveTtlPolicy,
    Bypass,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct OperationBrand(NonZeroU64);

impl OperationBrand {
    /// Called only by the private adapter after the operation admission has succeeded.
    pub(crate) fn from_admitted_operation(id: NonZeroU64) -> Self {
        Self(id)
    }

    fn id_for_test(&self) -> u64 {
        self.0.get()
    }
}

#[derive(Debug)]
enum ResolutionTicket {
    Positive {
        operation: OperationBrand,
        valid_until: Instant,
    },
    SingleOperation {
        operation: OperationBrand,
        original_resolve_end: Instant,
    },
}

impl ResolutionTicket {
    fn boundary(&self) -> Instant {
        match self {
            Self::Positive { valid_until, .. } => *valid_until,
            Self::SingleOperation {
                original_resolve_end,
                ..
            } => *original_resolve_end,
        }
    }

    fn cache_disposition(&self) -> CacheDisposition {
        match self {
            Self::Positive { .. } => CacheDisposition::PreserveExistingPositiveTtlPolicy,
            Self::SingleOperation { .. } => CacheDisposition::Bypass,
        }
    }

    fn operation_id(&self) -> u64 {
        match self {
            Self::Positive { operation, .. } | Self::SingleOperation { operation, .. } => {
                operation.id_for_test()
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct SuccessfulLookup {
    addresses: Vec<IpAddr>,
    ttl_chain_seconds: Vec<u32>,
    resolver_valid_until: Option<Instant>,
    original_resolve_end: Instant,
    operation_end: Instant,
    operation: OperationBrand,
}

impl SuccessfulLookup {
    /// The caller invokes this only after both required family results completed successfully.
    /// Empty family vectors represent NODATA. Any family error rejects the whole lookup.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_provider_results(
        operation: OperationBrand,
        ipv4: Result<Vec<Ipv4Addr>, ResolverFailure>,
        ipv6: Result<Vec<Ipv6Addr>, ResolverFailure>,
        ttl_chain_seconds: Vec<u32>,
        resolver_valid_until: Option<Instant>,
        original_resolve_end: Instant,
        operation_end: Instant,
    ) -> Result<Self, Q1Error> {
        let ipv4 = ipv4.map_err(|_| Q1Error::FamilyFailure)?;
        let ipv6 = ipv6.map_err(|_| Q1Error::FamilyFailure)?;
        if ttl_chain_seconds.is_empty() {
            return Err(Q1Error::EmptyTtlChain);
        }
        let count = ipv4
            .len()
            .checked_add(ipv6.len())
            .ok_or(Q1Error::Address(AddressError::TooManyCandidates))?;
        if count > MAX_CANDIDATES {
            return Err(Q1Error::Address(AddressError::TooManyCandidates));
        }
        let mut addresses = Vec::new();
        addresses
            .try_reserve_exact(count)
            .map_err(|_| Q1Error::Address(AddressError::Capacity))?;
        addresses.extend(ipv4.into_iter().map(IpAddr::V4));
        addresses.extend(ipv6.into_iter().map(IpAddr::V6));
        Ok(Self {
            addresses,
            ttl_chain_seconds,
            resolver_valid_until,
            original_resolve_end,
            operation_end,
            operation,
        })
    }

    pub(crate) fn check(
        self,
        policy: &InternalPolicy,
        expected_identity: ServiceIdentity,
        now: Instant,
    ) -> Result<CheckedResolution, Q1Error> {
        if now >= self.original_resolve_end {
            return Err(Q1Error::Address(AddressError::Expired));
        }
        let zero_ttl_in_chain = self.ttl_chain_seconds.contains(&0);
        let (ticket, effective_expiry) = if zero_ttl_in_chain {
            (
                ResolutionTicket::SingleOperation {
                    operation: self.operation,
                    original_resolve_end: self.original_resolve_end,
                },
                self.original_resolve_end,
            )
        } else {
            let valid_until = self
                .resolver_valid_until
                .ok_or(Q1Error::MissingPositiveExpiry)?;
            (
                ResolutionTicket::Positive {
                    operation: self.operation,
                    valid_until,
                },
                valid_until,
            )
        };
        let snapshot = check_candidates(
            policy,
            expected_identity,
            &self.addresses,
            effective_expiry,
            self.operation_end,
            now,
        )
        .map_err(Q1Error::Address)?;
        Ok(CheckedResolution { snapshot, ticket })
    }
}

#[derive(Debug)]
pub(crate) struct CheckedResolution {
    snapshot: CheckedSnapshot,
    ticket: ResolutionTicket,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AttemptControlStop {
    Closing,
    Cancelled,
    Fault(u8),
}

#[derive(Debug, Default)]
pub(crate) struct AttemptControl {
    closing: bool,
    cancelled: bool,
    first_fault: Option<u8>,
}

impl AttemptControl {
    pub(crate) fn close(&mut self) {
        self.closing = true;
    }

    pub(crate) fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub(crate) fn latch_fault(&mut self, fault: u8) {
        if self.first_fault.is_none() {
            self.first_fault = Some(fault);
        }
    }

    fn stopped(&self) -> Option<AttemptControlStop> {
        if let Some(fault) = self.first_fault {
            Some(AttemptControlStop::Fault(fault))
        } else if self.closing {
            Some(AttemptControlStop::Closing)
        } else if self.cancelled {
            Some(AttemptControlStop::Cancelled)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConnectRetryability {
    RetryNextCandidate,
    Terminal,
}

pub(crate) fn classify_connect_io(kind: ErrorKind) -> ConnectRetryability {
    match kind {
        ErrorKind::ConnectionRefused
        | ErrorKind::NetworkUnreachable
        | ErrorKind::HostUnreachable => ConnectRetryability::RetryNextCandidate,
        _ => ConnectRetryability::Terminal,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AttemptFailure {
    Io(ErrorKind),
    SlotExpired,
    Capacity,
    PeerMismatch,
    TlsFailure,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AttemptCleanup {
    Incomplete,
    SocketDisposedAndProvenNoChild,
    SocketDisposedAndChildJoined,
}

impl AttemptCleanup {
    fn is_complete(self) -> bool {
        matches!(
            self,
            Self::SocketDisposedAndProvenNoChild | Self::SocketDisposedAndChildJoined
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PlanStop {
    Closing,
    Cancelled,
    Fault(u8),
    OperationExpired,
    ConnectExpired,
    ResolutionExpired,
    SlotExpired,
    Exhausted,
    FatalConnect,
    CleanupIncomplete,
    InvalidState,
    Finished,
}

#[derive(Debug)]
pub(crate) struct DialAttempt {
    plan: Arc<DialPlanBrand>,
    index: usize,
    address: SocketAddr,
    slot_end: Instant,
}

#[derive(Debug)]
struct DialPlanBrand;

impl DialAttempt {
    pub(crate) fn address(&self) -> SocketAddr {
        self.address
    }

    pub(crate) fn slot_end(&self) -> Instant {
        self.slot_end
    }
}

#[derive(Debug)]
pub(crate) struct HttpEnqueueTicket {
    plan: Arc<DialPlanBrand>,
    operation_id: u64,
}

impl HttpEnqueueTicket {
    pub(crate) fn operation_id(&self) -> u64 {
        self.operation_id
    }
}

#[derive(Debug)]
pub(crate) struct DialPlan {
    brand: Arc<DialPlanBrand>,
    identity: ServiceIdentity,
    operation_end: Instant,
    ticket: ResolutionTicket,
    candidates: Vec<SocketAddr>,
    attempt_limit: usize,
    start: Instant,
    connect_budget: Duration,
    connect_end: Instant,
    cursor: usize,
    in_flight: Option<usize>,
    peer_checked: bool,
    tls_started: bool,
    tls_established: bool,
    enqueue_issued: bool,
    finished: bool,
}

impl DialPlan {
    pub(crate) fn checked(
        resolution: CheckedResolution,
        order: AttemptOrder,
        max_attempts: usize,
        connect_budget: Duration,
        start: Instant,
    ) -> Result<Self, Q1Error> {
        if max_attempts == 0 || max_attempts > MAX_CANDIDATES {
            return Err(Q1Error::InvalidAttemptLimit);
        }
        if connect_budget.is_zero() {
            return Err(Q1Error::InvalidConnectBudget);
        }
        let CheckedResolution { snapshot, ticket } = resolution;
        let port = snapshot.selected.port();
        let mut candidates = interleave(&snapshot.checked_candidates, port, order)?;
        let attempt_limit = max_attempts.min(candidates.len());
        connect_budget
            .as_nanos()
            .checked_mul(attempt_limit as u128)
            .ok_or(Q1Error::DeadlineArithmetic)?;
        candidates.truncate(attempt_limit);
        if attempt_limit == 0 {
            return Err(Q1Error::Address(AddressError::Empty));
        }
        let connect_end = start
            .checked_add(connect_budget)
            .ok_or(Q1Error::DeadlineArithmetic)?;
        if start >= snapshot.operation_end || start >= ticket.boundary() {
            return Err(Q1Error::Address(AddressError::Expired));
        }
        Ok(Self {
            brand: Arc::new(DialPlanBrand),
            identity: snapshot.identity,
            operation_end: snapshot.operation_end,
            ticket,
            candidates,
            attempt_limit,
            start,
            connect_budget,
            connect_end: connect_end.min(snapshot.operation_end),
            cursor: 0,
            in_flight: None,
            peer_checked: false,
            tls_started: false,
            tls_established: false,
            enqueue_issued: false,
            finished: false,
        })
    }

    // Only the owning plan can turn its live branded attempt into the numeric
    // socket authority. Preserve the original validity/operation ends unchanged.
    pub(super) fn dial_selection(
        &self,
        attempt: &DialAttempt,
    ) -> Result<super::DialSelection, PlanStop> {
        if !self.owns_attempt(attempt) || self.finished || self.peer_checked {
            return Err(PlanStop::InvalidState);
        }
        Ok(super::DialSelection {
            identity: self.identity,
            selected: attempt.address,
            valid_until: self.ticket.boundary(),
            operation_end: self.operation_end,
        })
    }

    pub(crate) fn next_attempt(
        &mut self,
        now: Instant,
        control: &AttemptControl,
    ) -> Result<DialAttempt, PlanStop> {
        self.check_control(control)?;
        self.check_whole_ends(now)?;
        if now >= self.ticket.boundary() {
            return self.stop(PlanStop::ResolutionExpired);
        }
        if self.in_flight.is_some() {
            return self.stop(PlanStop::InvalidState);
        }
        if self.finished || self.peer_checked {
            return Err(PlanStop::Finished);
        }
        while self.cursor < self.attempt_limit {
            let index = self.cursor;
            let slot_end = self.slot_end(index).map_err(|_| PlanStop::InvalidState)?;
            if now >= slot_end {
                self.cursor += 1;
                continue;
            }
            self.in_flight = Some(index);
            return Ok(DialAttempt {
                plan: Arc::clone(&self.brand),
                index,
                address: self.candidates[index],
                slot_end,
            });
        }
        self.stop(PlanStop::Exhausted)
    }

    pub(crate) fn fail_after_cleanup(
        &mut self,
        attempt: DialAttempt,
        failure: AttemptFailure,
        cleanup: AttemptCleanup,
        now: Instant,
        control: &AttemptControl,
    ) -> Result<(), PlanStop> {
        if !self.owns_attempt(&attempt) {
            return self.stop(PlanStop::InvalidState);
        }
        self.in_flight = None;
        if !cleanup.is_complete() {
            return self.stop(PlanStop::CleanupIncomplete);
        }
        self.check_control(control)?;
        self.check_whole_ends(now)?;
        if now >= self.ticket.boundary() {
            return self.stop(PlanStop::ResolutionExpired);
        }
        let retryable = match failure {
            AttemptFailure::Io(kind) => {
                classify_connect_io(kind) == ConnectRetryability::RetryNextCandidate
            }
            AttemptFailure::SlotExpired => {
                if now < attempt.slot_end {
                    return self.stop(PlanStop::InvalidState);
                }
                true
            }
            AttemptFailure::Capacity
            | AttemptFailure::PeerMismatch
            | AttemptFailure::TlsFailure
            | AttemptFailure::Unknown => false,
        };
        if !retryable {
            return self.stop(PlanStop::FatalConnect);
        }
        self.cursor = attempt.index.saturating_add(1);
        if self.cursor >= self.attempt_limit {
            return self.stop(PlanStop::Exhausted);
        }
        Ok(())
    }

    /// Called only after TCP connect and the exact peer-address check succeed.
    /// It intentionally does not recheck DNS TTL after the attempt's first poll.
    pub(crate) fn connected_after_peer_check(
        &mut self,
        attempt: DialAttempt,
        now: Instant,
        control: &AttemptControl,
    ) -> Result<(), PlanStop> {
        if !self.owns_attempt(&attempt) {
            return self.stop(PlanStop::InvalidState);
        }
        self.check_control(control)?;
        self.check_whole_ends(now)?;
        if now >= attempt.slot_end {
            return self.stop(PlanStop::SlotExpired);
        }
        self.in_flight = None;
        self.peer_checked = true;
        Ok(())
    }

    /// TLS is a terminal phase for candidate selection: starting it seals the plan.
    pub(crate) fn start_tls(
        &mut self,
        now: Instant,
        control: &AttemptControl,
    ) -> Result<(), Q1Error> {
        if !self.peer_checked || self.tls_started || self.finished {
            self.finished = true;
            return Err(Q1Error::PlanFinished);
        }
        self.check_late_stage_gate(now, control)?;
        self.tls_started = true;
        Ok(())
    }

    pub(crate) fn finish_tls(
        &mut self,
        now: Instant,
        control: &AttemptControl,
    ) -> Result<(), Q1Error> {
        if !self.tls_started || self.tls_established || self.finished {
            self.finished = true;
            return Err(Q1Error::PlanFinished);
        }
        self.check_late_stage_gate(now, control)?;
        self.tls_established = true;
        Ok(())
    }

    pub(crate) fn take_http_enqueue_ticket(
        &mut self,
        now: Instant,
        control: &AttemptControl,
    ) -> Result<HttpEnqueueTicket, Q1Error> {
        if !self.tls_established || self.finished {
            return Err(Q1Error::NotConnected);
        }
        self.check_late_stage_gate(now, control)?;
        if self.enqueue_issued {
            return Err(Q1Error::EnqueueAlreadyIssued);
        }
        self.enqueue_issued = true;
        Ok(HttpEnqueueTicket {
            plan: Arc::clone(&self.brand),
            operation_id: self.ticket.operation_id(),
        })
    }

    pub(crate) fn finish_http(&mut self, ticket: HttpEnqueueTicket) -> Result<(), Q1Error> {
        if self.finished
            || !self.enqueue_issued
            || !Arc::ptr_eq(&self.brand, &ticket.plan)
            || ticket.operation_id != self.ticket.operation_id()
        {
            self.finished = true;
            return Err(Q1Error::PlanFinished);
        }
        self.finished = true;
        Ok(())
    }

    fn slot_end(&self, index: usize) -> Result<Instant, Q1Error> {
        let numerator = index.checked_add(1).ok_or(Q1Error::DeadlineArithmetic)?;
        let scaled = self
            .connect_budget
            .as_nanos()
            .checked_mul(numerator as u128)
            .ok_or(Q1Error::DeadlineArithmetic)?;
        let denominator = self.attempt_limit as u128;
        let nanos = scaled / denominator + u128::from(scaled % denominator != 0);
        let seconds =
            u64::try_from(nanos / 1_000_000_000).map_err(|_| Q1Error::DeadlineArithmetic)?;
        let subsecond = (nanos % 1_000_000_000) as u32;
        let relative = Duration::new(seconds, subsecond);
        let end = self
            .start
            .checked_add(relative)
            .ok_or(Q1Error::DeadlineArithmetic)?;
        Ok(end.min(self.connect_end))
    }

    fn check_control(&mut self, control: &AttemptControl) -> Result<(), PlanStop> {
        match control.stopped() {
            Some(AttemptControlStop::Closing) => self.stop(PlanStop::Closing),
            Some(AttemptControlStop::Cancelled) => self.stop(PlanStop::Cancelled),
            Some(AttemptControlStop::Fault(fault)) => self.stop(PlanStop::Fault(fault)),
            None => Ok(()),
        }
    }

    fn check_whole_ends(&mut self, now: Instant) -> Result<(), PlanStop> {
        if now >= self.operation_end {
            self.stop(PlanStop::OperationExpired)
        } else if now >= self.connect_end {
            self.stop(PlanStop::ConnectExpired)
        } else {
            Ok(())
        }
    }

    fn owns_attempt(&self, attempt: &DialAttempt) -> bool {
        Arc::ptr_eq(&self.brand, &attempt.plan)
            && self.in_flight == Some(attempt.index)
            && self.candidates.get(attempt.index).copied() == Some(attempt.address)
            && self.slot_end(attempt.index).ok() == Some(attempt.slot_end)
    }

    fn check_late_stage_gate(
        &mut self,
        now: Instant,
        control: &AttemptControl,
    ) -> Result<(), Q1Error> {
        self.check_control(control).map_err(Q1Error::PlanStopped)?;
        if now >= self.operation_end {
            self.finished = true;
            return Err(Q1Error::PlanStopped(PlanStop::OperationExpired));
        }
        Ok(())
    }

    fn stop<T>(&mut self, reason: PlanStop) -> Result<T, PlanStop> {
        self.finished = true;
        Err(reason)
    }

    #[cfg(test)]
    fn identity_for_test(&self) -> ServiceIdentity {
        self.identity
    }

    #[cfg(test)]
    fn operation_id_for_test(&self) -> u64 {
        self.ticket.operation_id()
    }

    #[cfg(test)]
    fn candidate_count_for_test(&self) -> usize {
        self.candidates.len()
    }

    #[cfg(test)]
    fn operation_end_for_test(&self) -> Instant {
        self.operation_end
    }
}

fn interleave(
    candidates: &[IpAddr],
    port: u16,
    order: AttemptOrder,
) -> Result<Vec<SocketAddr>, Q1Error> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(candidates.len())
        .map_err(|_| Q1Error::Address(AddressError::Capacity))?;
    let mut v4 = candidates.iter().filter_map(|address| match address {
        IpAddr::V4(value) => Some(*value),
        IpAddr::V6(_) => None,
    });
    let mut v6 = candidates.iter().filter_map(|address| match address {
        IpAddr::V6(value) => Some(*value),
        IpAddr::V4(_) => None,
    });
    loop {
        let (first_v4, first_v6) = match order {
            AttemptOrder::Interleave {
                preferred: PreferredFamily::V4,
            } => (true, false),
            AttemptOrder::Interleave {
                preferred: PreferredFamily::V6,
            } => (false, true),
        };
        let mut added = false;
        if first_v4 {
            if let Some(address) = v4.next() {
                output.push(SocketAddr::new(IpAddr::V4(address), port));
                added = true;
            }
            if let Some(address) = v6.next() {
                output.push(SocketAddr::new(IpAddr::V6(address), port));
                added = true;
            }
        } else if first_v6 {
            if let Some(address) = v6.next() {
                output.push(SocketAddr::new(IpAddr::V6(address), port));
                added = true;
            }
            if let Some(address) = v4.next() {
                output.push(SocketAddr::new(IpAddr::V4(address), port));
                added = true;
            }
        }
        if !added {
            break;
        }
    }
    Ok(output)
}

#[cfg(test)]
#[path = "q1_dial_tests.rs"]
mod tests;
