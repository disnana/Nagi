//! Trusted Rust host startup boundary. No Nagi constructors or defaults.
//! The host must retain Owner and await shutdown on every teardown path.
use super::super::parent_drain::ParentError;
use super::*;
use address::socket_pin::tls_http1::FixedHeaders;
#[path = "retention.rs"]
mod retention;
pub use retention::RetentionLimits;
use retention::{header_charge, slots, sum, Retained, RetentionBudget};
pub(in crate::http_client) use retention::{FrozenTrust, RetentionClaim};
use std::mem::size_of;
#[cfg(test)]
use std::net::IpAddr;
use std::net::SocketAddr;

pub struct ServiceKey(ServiceIdentity);
impl ServiceKey {
    pub fn checked(
        service: u64,
        purpose: u64,
        generation: u64,
        trust: u64,
    ) -> Result<Self, Failure> {
        ServiceIdentity::from_startup(service, purpose, generation, trust)
            .map(Self)
            .map_err(|_| Failure::new(FailureKind::InvalidInput, false))
    }
}
pub struct ServiceConfig {
    pub key: ServiceKey,
    pub canonical_dns_name: String,
    pub port: u16,
    pub resolver_endpoint: SocketAddr,
    pub roots: rustls::RootCertStore,
    pub max_candidates: usize,
}
pub struct ServiceSpec {
    identity: ServiceIdentity,
    policy: Arc<InternalPolicy>,
    resolver: Arc<ResolverStartup>,
    max_candidates: usize,
    // Drops after the service/resolver/policy payloads that its claim covers.
    trust: Arc<FrozenTrust>,
}
impl ServiceSpec {
    pub fn checked(catalog: &StartupCatalog, config: ServiceConfig) -> Result<Self, Failure> {
        Self::checked_with_policy(catalog, config, Vec::new())
    }
    #[cfg(test)]
    pub(in crate::http_client) fn checked_for_test_network(
        catalog: &StartupCatalog,
        config: ServiceConfig,
        exact_exceptions: Vec<IpAddr>,
    ) -> Result<Self, Failure> {
        Self::checked_with_policy(catalog, config, exact_exceptions)
    }
    fn checked_with_policy(
        catalog: &StartupCatalog,
        config: ServiceConfig,
        exact_exceptions: Vec<std::net::IpAddr>,
    ) -> Result<Self, Failure> {
        let extra_config = sum(&[
            size_of::<Self>(),
            size_of::<InternalPolicy>(),
            size_of::<ResolverStartup>(),
            slots::<std::net::IpAddr>(exact_exceptions.capacity())?,
            config.canonical_dns_name.len(),
        ])?;
        let identity = config.key.0;
        let (trust, (policy, resolver)) = FrozenTrust::checked(
            &catalog.budget,
            identity,
            url::Host::Domain(config.canonical_dns_name),
            config.port,
            config.roots,
            extra_config,
            |host| {
                let url::Host::Domain(name) = host else {
                    return Err(Failure::invalid_input());
                };
                let resolver = Arc::new(
                    ResolverStartup::checked(name, config.resolver_endpoint)
                        .map_err(|_| Failure::invalid_input())?,
                );
                let policy = Arc::new(
                    InternalPolicy::checked(
                        identity,
                        config.port,
                        config.max_candidates,
                        exact_exceptions,
                    )
                    .map_err(|_| Failure::invalid_input())?,
                );
                Ok((policy, resolver))
            },
        )?;
        Ok(Self {
            identity,
            policy,
            resolver,
            max_candidates: config.max_candidates,
            trust,
        })
    }
}

// Explicit finite startup limits; all values and cross-field checks are required.
// Inherited native caps are the adopted initial support envelope, not defaults.
pub struct OperationLimits {
    pub jobs: usize,
    pub queue: usize,
    pub job_roots: usize,
    pub dns_roots: usize,
    pub dns_children: usize,
    pub http_drivers: usize,
    pub waiters: usize,
    pub waiter_bytes: usize,
    pub senders: usize,
    pub whole: Duration,
    pub connect: Duration,
    pub attempts: usize,
    pub input_bytes: usize,
    pub sockets: usize,
    pub max_headers: usize,
    pub max_header_bytes: usize,
    pub max_http_buffer_bytes: usize,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_target_bytes: usize,
}
impl OperationLimits {
    fn ledger(&self) -> Limits {
        Limits {
            jobs: self.jobs,
            queue: self.queue,
            job_roots: self.job_roots,
            dns_roots: self.dns_roots,
            dns_children: self.dns_children,
            http_drivers: self.http_drivers,
            waiters: self.waiters,
            waiter_bytes: self.waiter_bytes,
            senders: self.senders,
        }
    }
    fn transport(&self) -> Result<TransportLimits, Failure> {
        TransportLimits::checked(
            self.max_headers,
            self.max_header_bytes,
            self.max_http_buffer_bytes,
            self.max_request_bytes,
            self.max_response_bytes,
            self.max_target_bytes,
        )
        .map_err(|_| Failure::new(FailureKind::InvalidInput, false))
    }
    fn validate(&self) -> Result<(), Failure> {
        if !self.ledger().valid()
            || self.whole.is_zero()
            || self.connect.is_zero()
            || self.attempts == 0
            || self.input_bytes == 0
            || self.input_bytes > self.waiter_bytes
        {
            return Err(Failure::new(FailureKind::InvalidInput, false));
        }
        self.jobs
            .checked_mul(self.max_response_bytes)
            .and_then(|response| {
                self.jobs
                    .checked_mul(self.input_bytes)
                    .and_then(|input| response.checked_add(input))
            })
            .ok_or_else(|| Failure::new(FailureKind::InvalidInput, false))?;
        Instant::now()
            .checked_add(self.whole)
            .and_then(|_| Instant::now().checked_add(self.connect))
            .ok_or_else(|| Failure::new(FailureKind::InvalidInput, false))?;
        self.connect
            .as_nanos()
            .checked_mul(self.attempts as u128)
            .ok_or_else(|| Failure::new(FailureKind::InvalidInput, false))?;
        self.transport()?;
        SocketQuota::checked(self.sockets)
            .map_err(|_| Failure::new(FailureKind::InvalidInput, false))?;
        Ok(())
    }
}
pub struct FixedRequestHeaders {
    fixed: FixedHeaders,
    budget: Arc<RetentionBudget>,
}
impl FixedRequestHeaders {
    pub fn checked(
        catalog: &StartupCatalog,
        accept: Option<Vec<u8>>,
        content_type: Option<Vec<u8>>,
    ) -> Result<Self, Failure> {
        // All supplied capacities are checked together before native acceptance.
        let metadata = sum(&[size_of::<Self>(), size_of::<RetentionClaim>()])?;
        let charge = |value: &Option<Vec<u8>>| {
            value
                .as_ref()
                .map_or(Ok(0), |v| header_charge(v.capacity()))
        };
        let mut claim = catalog.budget.reserve(Retained {
            config: sum(&[metadata, charge(&accept)?, charge(&content_type)?])?,
            ..Retained::default()
        })?;
        let mut convert = |value: Vec<u8>| {
            let part = claim.split(Retained {
                config: header_charge(value.capacity())?,
                ..Retained::default()
            })?;
            retention::owned_header(value, part)
        };
        let accept = accept.map(&mut convert).transpose()?;
        let content_type = content_type.map(&mut convert).transpose()?;
        let fixed = FixedHeaders::from_retained(accept, content_type, None, Arc::new(claim));
        Ok(Self {
            fixed,
            budget: Arc::clone(&catalog.budget),
        })
    }
}
pub struct ServiceCredential {
    service: Arc<ServiceSpec>,
    operation: u64,
    value: hyper::header::HeaderValue,
}
impl ServiceCredential {
    pub fn bearer(
        catalog: &StartupCatalog,
        service: &Arc<ServiceSpec>,
        operation: u64,
        bytes: Vec<u8>,
    ) -> Result<Self, Failure> {
        if !Arc::ptr_eq(&catalog.budget, service.trust.budget())
            || operation == 0
            || bytes.is_empty()
        {
            return Err(Failure::new(FailureKind::InvalidInput, false));
        }
        let len = 7usize
            .checked_add(bytes.len())
            .ok_or_else(Failure::invalid_input)?;
        let mut claim = catalog.budget.reserve(Retained {
            config: sum(&[size_of::<Self>(), header_charge(len)?])?,
            ..Retained::default()
        })?;
        if bytes
            .iter()
            .any(|byte| !byte.is_ascii_alphanumeric() && !b"-._~+/=".contains(byte))
        {
            return Err(Failure::new(FailureKind::InvalidInput, false));
        }
        let mut value = Vec::new();
        value
            .try_reserve_exact(slots::<u8>(len)?)
            .map_err(|_| Failure::unavailable())?;
        claim.grow(Retained {
            config: value
                .capacity()
                .checked_sub(len)
                .ok_or_else(Failure::invalid_input)?,
            ..Retained::default()
        })?;
        value.extend_from_slice(b"Bearer ");
        value.extend_from_slice(&bytes);
        drop(bytes);
        let mut value = retention::owned_header(value, claim)?;
        value.set_sensitive(true);
        Ok(Self {
            service: Arc::clone(service),
            operation,
            value,
        })
    }
}
struct Spec {
    service: Arc<ServiceSpec>,
    operation: u64,
    method: Method,
    limits: OperationLimits,
    headers: FixedRequestHeaders,
    retained: Arc<RetentionClaim>,
}
pub struct ProtectedSpec<P> {
    spec: Spec,
    credential: Option<ServiceCredential>,
    permission: PhantomData<fn() -> P>,
}
pub struct PublicSpec {
    spec: Spec,
}
fn spec_checked(
    service: Arc<ServiceSpec>,
    operation: u64,
    method: Method,
    limits: OperationLimits,
    headers: FixedRequestHeaders,
) -> Result<Spec, Failure> {
    service.trust.budget().ensure_open()?;
    if !Arc::ptr_eq(service.trust.budget(), &headers.budget) {
        return Err(Failure::invalid_input());
    }
    limits.validate()?;
    if operation == 0 || method == Method::CONNECT || limits.attempts > service.max_candidates {
        return Err(Failure::new(FailureKind::InvalidInput, false));
    }
    // Method/URL are opaque native representations: charge their logical
    // retained inputs, plus the fixed Spec/Core metadata, before registration.
    let origin_charge = sum(&[
        8,
        match service.trust.host() {
            url::Host::Domain(n) => n.len(),
            _ => return Err(Failure::invalid_input()),
        },
        1,
        5,
        1,
    ])?;
    let retained = Arc::new(service.trust.budget().reserve(Retained {
        config: sum(&[
            size_of::<Spec>(),
            size_of::<Core>(),
            size_of::<RetentionClaim>(),
            method.as_str().len(),
            origin_charge,
        ])?,
        ..Retained::default()
    })?);
    Ok(Spec {
        service,
        operation,
        method,
        limits,
        headers,
        retained,
    })
}
impl<P> ProtectedSpec<P> {
    pub fn checked(
        service: Arc<ServiceSpec>,
        operation: u64,
        method: Method,
        limits: OperationLimits,
        headers: FixedRequestHeaders,
        credential: Option<ServiceCredential>,
    ) -> Result<Self, Failure> {
        if credential.as_ref().is_some_and(|value| {
            !Arc::ptr_eq(&value.service, &service) || value.operation != operation
        }) {
            return Err(Failure::new(FailureKind::InvalidInput, false));
        }
        Ok(Self {
            spec: spec_checked(service, operation, method, limits, headers)?,
            credential,
            permission: PhantomData,
        })
    }
}
impl PublicSpec {
    pub fn checked(
        service: Arc<ServiceSpec>,
        operation: u64,
        method: Method,
        limits: OperationLimits,
        headers: FixedRequestHeaders,
    ) -> Result<Self, Failure> {
        Ok(Self {
            spec: spec_checked(service, operation, method, limits, headers)?,
        })
    }
}

pub(in crate::http_client) struct ReadyState {
    value: AtomicBool,
    _claim: Arc<RetentionClaim>,
}
impl ReadyState {
    pub(super) fn load(&self, order: Ordering) -> bool {
        self.value.load(order)
    }
    fn store(&self, value: bool, order: Ordering) {
        self.value.store(value, order);
    }
    #[cfg(test)]
    pub(super) fn for_test(retained: &Arc<RetentionClaim>) -> Arc<Self> {
        Arc::new(Self {
            value: AtomicBool::new(true),
            _claim: Arc::clone(retained),
        })
    }
}
struct CatalogAuthority(Arc<RetentionBudget>);
impl Drop for CatalogAuthority {
    fn drop(&mut self) {
        self.0.close();
    }
}
pub struct StartupCatalog {
    parent: ParentDrain,
    ready: Arc<ReadyState>,
    identities: Vec<(ServiceIdentity, u64)>,
    capacity: usize,
    budget: Arc<RetentionBudget>,
    _authority: CatalogAuthority,
    // identities/ready are dropped before this reservation is returned.
    _claim: RetentionClaim,
}
pub struct Owner {
    parent: ParentDrain,
}
impl StartupCatalog {
    pub fn new(capacity: usize, retention: RetentionLimits) -> Result<Self, Failure> {
        let budget = RetentionBudget::new(retention)?;
        let mut claim = budget.reserve(Retained {
            config: sum(&[
                size_of::<Self>(),
                size_of::<ReadyState>(),
                size_of::<RetentionClaim>(),
                slots::<(ServiceIdentity, u64)>(capacity)?,
            ])?,
            ..Retained::default()
        })?;
        let parent = ParentDrain::new(capacity).map_err(|error| {
            Failure::new(
                match error {
                    ParentError::InvalidCapacity => FailureKind::InvalidInput,
                    ParentError::Allocation => FailureKind::Unavailable,
                    _ => FailureKind::Internal,
                },
                false,
            )
        })?;
        let mut identities = Vec::new();
        identities
            .try_reserve_exact(capacity)
            .map_err(|_| Failure::new(FailureKind::Unavailable, false))?;
        claim.grow(Retained {
            config: slots::<(ServiceIdentity, u64)>(identities.capacity())?
                .checked_sub(slots::<(ServiceIdentity, u64)>(capacity)?)
                .ok_or_else(Failure::invalid_input)?,
            ..Retained::default()
        })?;
        let ready_claim = claim.split(Retained {
            config: sum(&[size_of::<ReadyState>(), size_of::<RetentionClaim>()])?,
            ..Retained::default()
        })?;
        Ok(Self {
            parent,
            ready: Arc::new(ReadyState {
                value: AtomicBool::new(false),
                _claim: Arc::new(ready_claim),
            }),
            identities,
            capacity,
            _authority: CatalogAuthority(Arc::clone(&budget)),
            budget,
            _claim: claim,
        })
    }
    // Consuming freeze removes all mutable registration authority. The returned
    // owner retains the existing real monitor records, independently of handles.
    #[cfg(test)]
    pub(in crate::http_client) fn test_retained(&self) -> Arc<RetentionClaim> {
        Arc::new(
            self.budget
                .reserve(Retained {
                    config: sum(&[size_of::<RetentionClaim>(), size_of::<ReadyState>()]).unwrap(),
                    ..Retained::default()
                })
                .unwrap(),
        )
    }
    pub fn freeze(self) -> Owner {
        self.ready.store(true, Ordering::Release);
        self.budget.close();
        // Only catalog-owned buffers disappear here. Service/header/native
        // owners keep their own reservations until their actual final Drop.
        Owner {
            parent: self.parent,
        }
    }
    fn register(
        &mut self,
        spec: Spec,
        credential: Option<ServiceCredential>,
    ) -> Result<Arc<Core>, Failure> {
        self.budget.ensure_open()?;
        if !Arc::ptr_eq(&self.budget, spec.service.trust.budget())
            || !Arc::ptr_eq(&self.budget, &spec.headers.budget)
            || !Arc::ptr_eq(&self.budget, spec.retained.budget())
            || credential.as_ref().is_some_and(|value| {
                !Arc::ptr_eq(&value.service, &spec.service)
                    || value.operation != spec.operation
                    || !Arc::ptr_eq(&self.budget, value.service.trust.budget())
            })
        {
            return Err(Failure::invalid_input());
        }
        let identity = spec.service.identity;
        if self.identities.len() >= self.capacity
            || self.identities.contains(&(identity, spec.operation))
        {
            return Err(Failure::new(FailureKind::Capacity, false));
        }
        let transport = spec.limits.transport()?;
        let fixed_headers = spec
            .headers
            .fixed
            .with_authorization(credential.map(|value| value.value));
        let profile = NativeProfile {
            identity,
            policy: Arc::clone(&spec.service.policy),
            resolver: Arc::clone(&spec.service.resolver),
            transport,
            quota: SocketQuota::checked(spec.limits.sockets)
                .map_err(|_| Failure::invalid_input())?,
            whole: spec.limits.whole,
            connect: spec.limits.connect,
            attempts: spec.limits.attempts,
            input_bytes: spec.limits.input_bytes,
            method: spec.method,
            #[cfg(test)]
            driver_hold: None,
            trust: Arc::clone(&spec.service.trust),
            retained: spec.retained,
        };
        let origin = startup_origin(&profile)?;
        let authority = profile.trust.authority();
        fixed_headers
            .validate(&authority, &transport)
            .map_err(|_| Failure::invalid_input())?;
        let input_limits = InputLimits {
            max_bytes: profile.input_bytes,
        };
        let request_limits = RequestLimits {
            origin,
            max_body_bytes: transport.request_limit(),
            max_target_bytes: transport.path_limit(),
        };
        // All input/profile/secret validation precedes real parent registration.
        let (id, sender) = self
            .parent
            .register(spec.limits.ledger(), profile.input_bytes)
            .map_err(|_| Failure::new(FailureKind::Unavailable, false))?;
        let runner = self
            .parent
            .runner_owner(id)
            .map_err(|_| Failure::new(FailureKind::Internal, false))?;
        self.identities.push((identity, spec.operation));
        Ok(Arc::new(Core {
            sender,
            runner: Arc::new(runner),
            profile,
            input_limits,
            request_limits,
            ready: Arc::clone(&self.ready),
            operation_id: spec.operation,
            fixed_headers,
        }))
    }
}
impl Owner {
    pub fn close(&mut self) {
        for index in 0..self.parent.records.len() {
            let id = self.parent.records[index].id;
            let _ = self.parent.request_close(id);
        }
    }
    pub async fn shutdown(&mut self) {
        self.parent.shutdown().await;
    }
}
pub fn register_protected<C: Send + Sync + 'static, P, I>(
    startup: &mut StartupCatalog,
    spec: ProtectedSpec<P>,
    state: Arc<C>,
    freeze: fn(I, &InputLimits) -> Result<FrozenInput, Failure>,
    prepare: fn(&C, BoundValues, FrozenInput, &RequestLimits) -> Result<PreparedRequest, Failure>,
) -> Result<ProtectedOperation<C, P, I>, Failure> {
    let core = startup.register(spec.spec, spec.credential)?;
    Ok(ProtectedOperation {
        core,
        state,
        freeze,
        prepare,
        permission: PhantomData,
    })
}
pub fn register_public<C: Send + Sync + 'static, I>(
    startup: &mut StartupCatalog,
    spec: PublicSpec,
    state: Arc<C>,
    freeze: fn(I, &InputLimits) -> Result<FrozenInput, Failure>,
    prepare: fn(&C, FrozenInput, &RequestLimits) -> Result<PreparedRequest, Failure>,
) -> Result<PublicOperation<C, I>, Failure> {
    let core = startup.register(spec.spec, None)?;
    Ok(PublicOperation {
        core,
        state,
        freeze,
        prepare,
    })
}
