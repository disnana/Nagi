use std::{
    future::{poll_fn, Future},
    mem::{size_of, size_of_val},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

use nagi_runtime::{
    auth::{AuthScope, Failure, Grant, VerifiedIdentity},
    http_server as http,
    metrics::{self, Allocations},
    serde_json::{self, json},
};

struct FutureTotals {
    calls: AtomicU64,
    future_bytes: AtomicU64,
    polls: AtomicU64,
    allocations: AtomicU64,
    reallocations: AtomicU64,
    allocated_bytes: AtomicU64,
    deallocations: AtomicU64,
}

impl FutureTotals {
    const fn new() -> Self {
        Self {
            calls: AtomicU64::new(0),
            future_bytes: AtomicU64::new(0),
            polls: AtomicU64::new(0),
            allocations: AtomicU64::new(0),
            reallocations: AtomicU64::new(0),
            allocated_bytes: AtomicU64::new(0),
            deallocations: AtomicU64::new(0),
        }
    }

    fn record(&self, observation: FutureObservation) {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.future_bytes
            .fetch_add(observation.future_bytes as u64, Ordering::Relaxed);
        self.polls
            .fetch_add(observation.polls, Ordering::Relaxed);
        self.allocations
            .fetch_add(observation.allocations.allocations, Ordering::Relaxed);
        self.reallocations
            .fetch_add(observation.allocations.reallocations, Ordering::Relaxed);
        self.allocated_bytes.fetch_add(
            observation.allocations.allocated_bytes,
            Ordering::Relaxed,
        );
        self.deallocations.fetch_add(
            observation.allocations.deallocations,
            Ordering::Relaxed,
        );
    }

    fn report(&self) -> serde_json::Value {
        let calls = self.calls.load(Ordering::Relaxed);
        let future_bytes = self.future_bytes.load(Ordering::Relaxed);
        json!({
            "calls": calls,
            "mean_future_bytes": if calls == 0 { 0.0 } else { future_bytes as f64 / calls as f64 },
            "polls": self.polls.load(Ordering::Relaxed),
            "poll_allocations": self.allocations.load(Ordering::Relaxed),
            "poll_reallocations": self.reallocations.load(Ordering::Relaxed),
            "poll_allocated_bytes": self.allocated_bytes.load(Ordering::Relaxed),
            "poll_deallocations": self.deallocations.load(Ordering::Relaxed),
            "allocation_scope": "calling thread during Future::poll only; excludes Future object storage"
        })
    }
}

struct ScalarTotals {
    count: AtomicU64,
    sum: AtomicU64,
    max: AtomicU64,
}

impl ScalarTotals {
    const fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
            sum: AtomicU64::new(0),
            max: AtomicU64::new(0),
        }
    }

    fn record(&self, value: u64) {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum.fetch_add(value, Ordering::Relaxed);
        self.max.fetch_max(value, Ordering::Relaxed);
    }

    fn report(&self) -> serde_json::Value {
        let count = self.count.load(Ordering::Relaxed);
        let sum = self.sum.load(Ordering::Relaxed);
        json!({
            "count": count,
            "mean": if count == 0 { 0.0 } else { sum as f64 / count as f64 },
            "max": self.max.load(Ordering::Relaxed)
        })
    }
}

struct AllocationTotals {
    calls: AtomicU64,
    allocations: AtomicU64,
    reallocations: AtomicU64,
    allocated_bytes: AtomicU64,
    deallocations: AtomicU64,
}

impl AllocationTotals {
    const fn new() -> Self {
        Self {
            calls: AtomicU64::new(0),
            allocations: AtomicU64::new(0),
            reallocations: AtomicU64::new(0),
            allocated_bytes: AtomicU64::new(0),
            deallocations: AtomicU64::new(0),
        }
    }

    fn record(&self, allocations: Allocations) {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.allocations
            .fetch_add(allocations.allocations, Ordering::Relaxed);
        self.reallocations
            .fetch_add(allocations.reallocations, Ordering::Relaxed);
        self.allocated_bytes
            .fetch_add(allocations.allocated_bytes, Ordering::Relaxed);
        self.deallocations
            .fetch_add(allocations.deallocations, Ordering::Relaxed);
    }

    fn report(&self) -> serde_json::Value {
        json!({
            "calls": self.calls.load(Ordering::Relaxed),
            "allocations": self.allocations.load(Ordering::Relaxed),
            "reallocations": self.reallocations.load(Ordering::Relaxed),
            "allocated_bytes": self.allocated_bytes.load(Ordering::Relaxed),
            "deallocations": self.deallocations.load(Ordering::Relaxed),
            "scope": "synchronous call on calling thread"
        })
    }
}

#[derive(Clone, Copy)]
struct FutureObservation {
    future_bytes: usize,
    polls: u64,
    allocations: Allocations,
}

static VERIFY_FUTURES: FutureTotals = FutureTotals::new();
static POLICY_FUTURES: FutureTotals = FutureTotals::new();
static READ_FUTURES: FutureTotals = FutureTotals::new();
static VERIFY_CALLBACK_BYTES: ScalarTotals = ScalarTotals::new();
static AUTHORIZE_CALLBACK_BYTES: ScalarTotals = ScalarTotals::new();
static READ_CALLBACK_BYTES: ScalarTotals = ScalarTotals::new();
static POLICY_CALLBACK_BYTES: ScalarTotals = ScalarTotals::new();
static GRANT_MINT_ALLOCATIONS: AllocationTotals = AllocationTotals::new();
static SCOPE_CALLBACK_NS: ScalarTotals = ScalarTotals::new();
static GRANT_MINT_TO_READ_NS: ScalarTotals = ScalarTotals::new();
static GRANT_MINT_TO_SUBMIT_NS: ScalarTotals = ScalarTotals::new();
static READ_ENTRY_TO_SUBMIT_NS: ScalarTotals = ScalarTotals::new();
static GRANT_SUBMIT_NS: ScalarTotals = ScalarTotals::new();
static GRANT_MINT: Mutex<Option<Instant>> = Mutex::new(None);
static READ_CAPACITY: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();

async fn measure_future<F: Future>(future: F) -> (F::Output, FutureObservation) {
    let future_bytes = size_of_val(&future);
    let mut polls = 0;
    let mut allocation_totals = Allocations::default();
    let mut pinned = std::pin::pin!(future);
    let output = poll_fn(|context| {
        let (poll, allocations) = metrics::measure(|| pinned.as_mut().poll(context));
        polls += 1;
        allocation_totals.allocations += allocations.allocations;
        allocation_totals.reallocations += allocations.reallocations;
        allocation_totals.allocated_bytes += allocations.allocated_bytes;
        allocation_totals.deallocations += allocations.deallocations;
        poll
    })
    .await;
    (
        output,
        FutureObservation {
            future_bytes,
            polls,
            allocations: allocation_totals,
        },
    )
}

async fn observe_future<F: Future>(future: F, totals: &'static FutureTotals) -> F::Output {
    let (output, observation) = measure_future(future).await;
    totals.record(observation);
    output
}

fn record_grant_mint_allocations(allocations: Allocations) {
    GRANT_MINT_ALLOCATIONS.record(allocations);
}

pub fn initialize() {
    READ_CAPACITY
        .set(Arc::new(tokio::sync::Semaphore::new(1)))
        .expect("SF01 fixture read capacity initialized once");
}

async fn verify_inner(
    request: http::Request,
    _state: Arc<super::State>,
) -> Result<VerifiedIdentity, Failure> {
    if !request.body().is_empty() {
        return Err(Failure::invalid_request());
    }
    let credential = http::header_text(&request, "authorization")
        .map_err(|_| Failure::invalid_request())?;
    if credential != Some("Bearer sf01-fixture") {
        return Err(Failure::invalid_credential());
    }
    VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(30))
}

pub fn verify(
    request: http::Request,
    state: Arc<super::State>,
) -> impl Future<Output = Result<VerifiedIdentity, Failure>> + Send + 'static {
    let future = observe_future(verify_inner(request, state), &VERIFY_FUTURES);
    VERIFY_CALLBACK_BYTES.record(size_of_val(&future) as u64);
    future
}

pub async fn policy_pause() {
    tokio::task::yield_now().await;
}

pub(super) async fn measure_policy<F: Future<Output = Result<(), Failure>>>(
    future: F,
) -> Result<(), Failure> {
    let future_bytes = size_of_val(&future) as u64;
    POLICY_CALLBACK_BYTES.record(future_bytes);
    observe_future(future, &POLICY_FUTURES).await
}

pub(super) fn resource_id(request: &http::Request) -> Result<i64, Failure> {
    request
        .path()
        .strip_prefix("/documents/")
        .and_then(|value| value.parse().ok())
        .ok_or_else(Failure::invalid_request)
}

pub(super) fn issue_grant(
    scope: AuthScope,
    resource: i64,
    entered: Instant,
) -> Result<Grant<super::Read>, Failure> {
    let (grant, allocations) = metrics::measure(|| Grant::from_authorized(scope, resource));
    record_grant_mint_allocations(allocations);
    let grant = grant?;
    if let Ok(mut mint) = GRANT_MINT.lock() {
        *mint = Some(Instant::now());
    }
    SCOPE_CALLBACK_NS.record(entered.elapsed().as_nanos() as u64);
    Ok(grant)
}

async fn read_document_inner(
    grant: Grant<super::Read>,
) -> Result<super::Document, Failure> {
    let read_started = Instant::now();
    let minted = GRANT_MINT
        .lock()
        .map_err(|_| Failure::internal())?
        .take()
        .ok_or_else(Failure::internal)?;
    GRANT_MINT_TO_READ_NS.record(minted.elapsed().as_nanos() as u64);
    let capacity = READ_CAPACITY.get().cloned().ok_or_else(Failure::internal)?;
    let reservation = tokio::time::timeout(Duration::from_secs(1), capacity.acquire_owned())
        .await
        .map_err(|_| Failure::unavailable())?
        .map_err(|_| Failure::unavailable())?;
    let submit_started = Instant::now();
    let result = grant.submit(reservation, |subject, resource, reservation| {
        drop(reservation);
        if subject != 7 || resource != 1 {
            return Err(Failure::denied());
        }
        Ok(super::Document {
            id: resource,
            title: "authorized fixture".to_owned(),
        })
    });
    let submit_ns = submit_started.elapsed().as_nanos() as u64;
    GRANT_SUBMIT_NS.record(submit_ns);
    READ_ENTRY_TO_SUBMIT_NS.record(read_started.elapsed().as_nanos() as u64);
    GRANT_MINT_TO_SUBMIT_NS.record(minted.elapsed().as_nanos() as u64);
    let admitted = result?;
    admitted
}

pub fn read_document(
    grant: Grant<super::Read>,
) -> impl Future<Output = Result<super::Document, Failure>> + Send + 'static {
    let future = observe_future(read_document_inner(grant), &READ_FUTURES);
    READ_CALLBACK_BYTES.record(size_of_val(&future) as u64);
    future
}

pub fn report() {
    let erased_policy_future_bytes = size_of::<
        std::pin::Pin<
            Box<
                dyn Future<Output = Result<Grant<super::Read>, Failure>>
                    + Send
                    + 'static,
            >,
        >,
    >();
    eprintln!(
        "SF01_METRICS {}",
        json!({
            "mode": POLICY_MODE,
            "objects": {
                "verified_identity_bytes": size_of::<VerifiedIdentity>(),
                "auth_scope_bytes": size_of::<AuthScope>(),
                "grant_bytes": size_of::<Grant<super::Read>>(),
                "erased_dispatcher_policy_future_handle_bytes": erased_policy_future_bytes,
                "lease_heap_bytes_measured": false
            },
            "futures": {
                "verifier_inner": VERIFY_FUTURES.report(),
                "verifier_callback": VERIFY_CALLBACK_BYTES.report(),
                "policy_inner": POLICY_FUTURES.report(),
                "policy_callback": POLICY_CALLBACK_BYTES.report(),
                "authorize_callback": AUTHORIZE_CALLBACK_BYTES.report(),
                "read_inner": READ_FUTURES.report(),
                "read_callback": READ_CALLBACK_BYTES.report(),
                "outer_dispatcher_policy_future_measured": false
            },
            "allocations": {
                "grant_mint_synchronous": GRANT_MINT_ALLOCATIONS.report(),
                "policy_future_poll": POLICY_FUTURES.report(),
                "read_future_poll": READ_FUTURES.report()
            },
            "visible_lifetimes_ns": {
                "scope_from_authorizer_entry_to_grant_mint": SCOPE_CALLBACK_NS.report(),
                "grant_mint_to_read_adapter_entry": GRANT_MINT_TO_READ_NS.report(),
                "grant_mint_to_submit": GRANT_MINT_TO_SUBMIT_NS.report(),
                "read_adapter_entry_to_submit": READ_ENTRY_TO_SUBMIT_NS.report(),
                "grant_submit_call": GRANT_SUBMIT_NS.report()
            },
            "measurements": {
                "max_simultaneous_requests": 1,
                "loopback_only": true,
                "policy_future_polls_use_runtime_metrics_measure": true,
                "no_lease_refcount_or_full_request_lifetime_probe": true
            }
        })
    );
}
