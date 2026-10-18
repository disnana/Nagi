//! Finite rate attachment to existing route slots. Existing routing paths,
//! opaque matcher/hash-table allocations, handler captures/shared state and
//! allocator/RSS are outside this new rate catalogue's storage domain.
use super::*;
use crate::security_sf07_private::{indexed::RateDecision, startup_rate as core};
use response_budget::wire::FallbackStatus;
use std::mem::size_of;

pub struct RateLimits(core::Capacity);
impl fmt::Debug for RateLimits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RateLimits { .. }")
    }
}
pub struct RatePolicy(core::Policy);
fn error(error: core::Error) -> Error {
    match error {
        core::Error::Invalid => Error::invalid("invalid HTTP rate limits or policy"),
        core::Error::Allocation => Error::internal("HTTP rate allocation unavailable"),
        core::Error::Fault => Error::internal("HTTP rate invariant failure"),
    }
}
pub fn rate_limits(
    rules: i64,
    domains: i64,
    subject_slots: i64,
    retained_bytes: i64,
    request_work: i64,
) -> Result<RateLimits, Error> {
    core::Capacity::checked(rules, domains, subject_slots, retained_bytes, request_work)
        .map(RateLimits)
        .map_err(error)
}
pub fn with_rate_limits(mut options: Options, limits: RateLimits) -> Result<Options, Error> {
    options.rate_limits = Some(limits);
    validate_options(&options)?;
    Ok(options)
}
pub fn rate_policy(burst: i64, refill_ms: i64) -> Result<RatePolicy, Error> {
    core::Policy::checked(burst, refill_ms)
        .map(RatePolicy)
        .map_err(error)
}
pub fn rate_subject(
    policy: RatePolicy,
    burst: i64,
    refill_ms: i64,
    subject_slots: i64,
    idle_ms: i64,
) -> Result<RatePolicy, Error> {
    policy
        .0
        .with_subject(burst, refill_ms, subject_slots, idle_ms)
        .map(RatePolicy)
        .map_err(error)
}
pub fn rate_route<S, E>(
    mut app: App<S, E>,
    method: Method,
    path: &str,
    policy: RatePolicy,
) -> Result<App<S, E>, Error> {
    let route = app
        .routes
        .get_mut(path)
        .and_then(|routes| routes.iter_mut().find(|route| route.method == method))
        .ok_or_else(|| {
            Error::invalid("rate attachment requires an exact registered method/path")
        })?;
    if route.rate.is_some() {
        return Err(Error::invalid("HTTP route rate policy is already attached"));
    }
    if policy.0.has_subject() && !verified(route.source) {
        return Err(Error::invalid(
            "subject rate policy requires a verified credential route",
        ));
    }
    route.rate = Some(policy);
    Ok(app)
}
fn verified(source: PolicySource) -> bool {
    matches!(
        source,
        PolicySource::Bearer | PolicySource::Session | PolicySource::BrowserLogin
    )
}
fn add(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b)
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| Error::invalid("HTTP rate catalogue overflow"))
}
fn array<T>(count: usize) -> Result<usize, Error> {
    count
        .checked_mul(size_of::<T>())
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| Error::invalid("HTTP rate catalogue overflow"))
}
fn fixed_metadata(requests: usize, connections: usize) -> Result<usize, Error> {
    let owner = add(size_of::<RuntimeRate>(), 2 * size_of::<usize>())?;
    let leases = crate::auth::rate_metadata_bytes(requests)
        .ok_or_else(|| Error::invalid("HTTP request rate metadata overflow"))?;
    // Full actual received-owner stride is conservative credit for the inline
    // rate slot and shared owner/control. Live admitted retained Requests are
    // bounded by requests; not-yet-admitted owners are bounded by connections.
    let headers = array::<request_headers::ReceivedHeaders>(add(requests, connections)?)?;
    let controls = array::<[usize; 2]>(add(requests, connections)?)?;
    // One construction temporary per connection; service closure and its one
    // HTTP/1 request future each retain an Arc, plus the server's own Arc. No
    // Context clone carries RequestRate, and no second catalogue is built.
    let temporaries = array::<RequestRate>(connections)?;
    let handles = array::<Arc<RuntimeRate>>(add(add(connections, connections)?, 1)?)?;
    let config = size_of::<Option<RateLimits>>();
    add(
        owner,
        add(
            leases,
            add(
                headers,
                add(controls, add(temporaries, add(handles, config)?)?)?,
            )?,
        )?,
    )
}
pub(super) fn validate_options(options: &Options) -> Result<(), Error> {
    if let Some(limits) = &options.rate_limits {
        limits
            .0
            .admit_catalogue(0, fixed_metadata(options.requests, options.connections)?)
            .map_err(error)?;
    }
    Ok(())
}
pub(super) struct RuntimeRate {
    plan: Option<core::Plan>,
}
impl RuntimeRate {
    pub(super) fn checked<S, E>(
        app: &mut App<S, E>,
        limits: &RateLimits,
        requests: usize,
        connections: usize,
    ) -> Result<Arc<Self>, Error> {
        let mut count = 0usize;
        // Charge the ACTUAL entire route Vec stride conservatively, avoiding
        // assumptions about old/new repr(Rust) padding. No routing graph clone.
        let mut catalogue = fixed_metadata(requests, connections)?;
        for routes in app.routes.values() {
            catalogue = add(catalogue, array::<Route<S>>(routes.capacity())?)?;
            count = add(count, routes.len())?;
            for route in routes {
                let policy = route.rate.as_ref().ok_or_else(|| {
                    Error::invalid("every HTTP route requires an explicit rate policy")
                })?;
                if policy.0.has_subject() && !verified(route.source) {
                    return Err(Error::invalid(
                        "subject rate policy requires a verified credential route",
                    ));
                }
            }
        }
        let planned = add(catalogue, array::<core::Record>(count)?)?;
        limits.0.admit_catalogue(count, planned).map_err(error)?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(count)
            .map_err(|_| Error::internal("HTTP rate catalogue allocation unavailable"))?;
        // Retain temporary Record capacity in the credit through Plan startup,
        // even after the temporary eventually drops.
        catalogue = add(catalogue, array::<core::Record>(records.capacity())?)?;
        limits.0.admit_catalogue(count, catalogue).map_err(error)?;
        for route in app.routes.values_mut().flatten() {
            route.rate_index = Some(records.len());
            let support = if verified(route.source) {
                core::SubjectSupport::Verified
            } else {
                core::SubjectSupport::None
            };
            records.push(
                route
                    .rate
                    .as_ref()
                    .expect("validated exact route policy")
                    .0
                    .record(support),
            );
        }
        let plan = core::Plan::checked_ref(&records, &limits.0, catalogue).map_err(error)?;
        if let Some(plan) = &plan {
            limits
                .0
                .admit_catalogue(count, plan.retained_bytes())
                .map_err(error)?;
        }
        Ok(Arc::new(Self { plan }))
    }
    pub(super) fn request(self: &Arc<Self>, record: usize) -> Result<RequestRate, Error> {
        Ok(RequestRate {
            runtime: Arc::clone(self),
            binding: self
                .plan
                .as_ref()
                .and_then(|plan| plan.binding(record))
                .ok_or_else(|| Error::internal("HTTP rate binding missing"))?,
        })
    }
    pub(super) fn close(&self) {
        if let Some(plan) = &self.plan {
            plan.close();
        }
    }
}
#[derive(Clone)]
pub(super) struct RequestRate {
    runtime: Arc<RuntimeRate>,
    binding: core::Binding,
}
fn decision(value: RateDecision) -> Result<(), FallbackStatus> {
    match value {
        RateDecision::Charged => Ok(()),
        RateDecision::Limited { retry_ns } if retry_ns > 0 => Err(FallbackStatus::Limited {
            retry_seconds: retry_ns / 1_000_000_000 + u64::from(retry_ns % 1_000_000_000 != 0),
        }),
        RateDecision::Capacity | RateDecision::Closing | RateDecision::ClockFault => {
            Err(FallbackStatus::Capacity)
        }
        _ => Err(FallbackStatus::Internal),
    }
}
enum SubjectFailure {
    Authority(Failure),
    Rate(FallbackStatus),
}
impl RequestRate {
    pub(super) fn route(&self) -> Result<(), FallbackStatus> {
        decision(
            self.runtime
                .plan
                .as_ref()
                .expect("checked route binding owner")
                .charge_route(self.binding),
        )
    }
    fn subject(&self, lease: &Lease) -> Result<(), SubjectFailure> {
        if !self.binding.has_subject() {
            return Ok(());
        }
        let subject = lease.rate_subject().map_err(SubjectFailure::Authority)?;
        decision(
            self.runtime
                .plan
                .as_ref()
                .expect("checked route binding owner")
                .charge_subject(self.binding, subject),
        )
        .map_err(SubjectFailure::Rate)
    }
}

pub(super) async fn before_handler(
    context: &ResponseContext,
    rate: Option<&RequestRate>,
    lease: &Arc<Lease>,
    source: PolicySource,
    head: bool,
) -> Option<HttpResponse> {
    if let Err(failure) = lease.validate() {
        return Some(security_response(context, failure, source, head));
    }
    let Some(rate) = rate else {
        return Some(HttpResponse::rate_refusal(
            context,
            FallbackStatus::Internal,
            source,
            head,
        ));
    };
    match rate.subject(lease) {
        Ok(()) => {}
        Err(SubjectFailure::Authority(failure)) => {
            return Some(security_response(context, failure, source, head))
        }
        Err(SubjectFailure::Rate(reason)) => {
            return Some(HttpResponse::rate_refusal(context, reason, source, head))
        }
    }
    let completion = lease.finish_http_auth();
    #[cfg(nagi_auth_stage_diag)]
    let completion = crate::auth::session_deadline_observer::observe_guarded(
        crate::auth::session_deadline_observer::Phase::DeferredTouch,
        completion,
    );
    completion
        .await
        .err()
        .map(|failure| security_response(context, failure, source, head))
}

#[cfg(test)]
#[path = "rate_public_tests.rs"]
mod tests;

// Explicit finite unit fixture; no production default or Context constructor.
#[cfg(test)]
pub(super) fn fixture_request_rate() -> RequestRate {
    let records = [core::Policy::checked(1, 60_000)
        .unwrap()
        .record(core::SubjectSupport::None)];
    let limits = core::Capacity::checked(1, 1, 1, 1024 * 1024, 10_000).unwrap();
    let catalogue = size_of::<RuntimeRate>() + 2 * size_of::<usize>() + size_of::<core::Record>();
    let runtime = Arc::new(RuntimeRate {
        plan: core::Plan::checked_ref(&records, &limits, catalogue).unwrap(),
    });
    runtime.request(0).unwrap()
}
