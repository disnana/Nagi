//! Trusted Rust host startup boundary. No Nagi constructors or defaults.
//! The host must retain Owner and await shutdown on every teardown path.
use super::*;
use address::socket_pin::tls_http1::FixedHeaders;
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
    name: String,
    port: u16,
    endpoint: SocketAddr,
    roots: rustls::RootCertStore,
    policy: Arc<InternalPolicy>,
    max_candidates: usize,
}
impl ServiceSpec {
    // Carry the audited narrow provider profile: exact canonical DNS name,
    // Explicit endpoint/port/roots with the existing default-deny address policy.
    // Production startup cannot authorize special/internal addresses.
    pub fn checked(config: ServiceConfig) -> Result<Self, Failure> {
        Self::checked_with_policy(config, Vec::new())
    }

    // Test-network-admission seam only: TLS, Grant, owner and actual native joins
    // still execute normally. This does not establish a production service profile.
    #[cfg(test)]
    pub(in crate::http_client) fn checked_for_test_network(
        config: ServiceConfig,
        exact_exceptions: Vec<IpAddr>,
    ) -> Result<Self, Failure> {
        Self::checked_with_policy(config, exact_exceptions)
    }

    fn checked_with_policy(
        config: ServiceConfig,
        exact_exceptions: Vec<std::net::IpAddr>,
    ) -> Result<Self, Failure> {
        ResolverStartup::checked(&config.canonical_dns_name, config.resolver_endpoint)
            .map_err(|_| Failure::new(FailureKind::InvalidInput, false))?;
        let identity = config.key.0;
        let policy = Arc::new(
            InternalPolicy::checked(
                identity,
                config.port,
                config.max_candidates,
                exact_exceptions,
            )
            .map_err(|_| Failure::new(FailureKind::InvalidInput, false))?,
        );
        Ok(Self {
            identity,
            name: config.canonical_dns_name,
            port: config.port,
            endpoint: config.resolver_endpoint,
            roots: config.roots,
            policy,
            max_candidates: config.max_candidates,
        })
    }
}

// Explicit native test/development configuration. These private inherited caps
// are not advertised public support limits and there is no Default impl.
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
    accept: Option<Vec<u8>>,
    content_type: Option<Vec<u8>>,
}
impl FixedRequestHeaders {
    pub fn checked(
        accept: Option<Vec<u8>>,
        content_type: Option<Vec<u8>>,
    ) -> Result<Self, Failure> {
        FixedHeaders::checked(accept.clone(), content_type.clone(), None)
            .map_err(|_| Failure::new(FailureKind::InvalidInput, false))?;
        Ok(Self {
            accept,
            content_type,
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
        service: &Arc<ServiceSpec>,
        operation: u64,
        bytes: Vec<u8>,
    ) -> Result<Self, Failure> {
        if operation == 0
            || bytes.is_empty()
            || bytes
                .iter()
                .any(|byte| !byte.is_ascii_alphanumeric() && !b"-._~+/=".contains(byte))
        {
            return Err(Failure::new(FailureKind::InvalidInput, false));
        }
        let mut value = Vec::new();
        value
            .try_reserve_exact(
                7usize
                    .checked_add(bytes.len())
                    .ok_or_else(|| Failure::new(FailureKind::InvalidInput, false))?,
            )
            .map_err(|_| Failure::new(FailureKind::Unavailable, false))?;
        value.extend_from_slice(b"Bearer ");
        value.extend_from_slice(&bytes);
        let mut value = hyper::header::HeaderValue::from_bytes(&value)
            .map_err(|_| Failure::new(FailureKind::InvalidInput, false))?;
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
    limits.validate()?;
    if operation == 0 || method == Method::CONNECT || limits.attempts > service.max_candidates {
        return Err(Failure::new(FailureKind::InvalidInput, false));
    }
    Ok(Spec {
        service,
        operation,
        method,
        limits,
        headers,
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

pub struct StartupCatalog {
    parent: ParentDrain,
    ready: Arc<AtomicBool>,
    identities: Vec<(ServiceIdentity, u64)>,
    capacity: usize,
}
pub struct Owner {
    parent: ParentDrain,
}
impl StartupCatalog {
    pub fn new(capacity: usize) -> Result<Self, Failure> {
        let parent = ParentDrain::new(capacity)
            .map_err(|_| Failure::new(FailureKind::InvalidInput, false))?;
        let mut identities = Vec::new();
        identities
            .try_reserve_exact(capacity)
            .map_err(|_| Failure::new(FailureKind::Unavailable, false))?;
        Ok(Self {
            parent,
            ready: Arc::new(AtomicBool::new(false)),
            identities,
            capacity,
        })
    }
    // Consuming freeze removes all mutable registration authority. The returned
    // owner retains the existing real monitor records, independently of handles.
    pub fn freeze(self) -> Owner {
        self.ready.store(true, Ordering::Release);
        Owner {
            parent: self.parent,
        }
    }
    fn register(
        &mut self,
        spec: Spec,
        credential: Option<ServiceCredential>,
    ) -> Result<Arc<Core>, Failure> {
        let identity = spec.service.identity;
        if self.identities.len() >= self.capacity
            || self.identities.contains(&(identity, spec.operation))
        {
            return Err(Failure::new(FailureKind::Capacity, false));
        }
        let transport = spec.limits.transport()?;
        let fixed_headers = FixedHeaders::checked(
            spec.headers.accept,
            spec.headers.content_type,
            credential.map(|value| value.value),
        )
        .map_err(|_| Failure::new(FailureKind::InvalidInput, false))?;
        let profile = NativeProfile {
            identity,
            policy: Arc::clone(&spec.service.policy),
            resolver: ResolverStartup::checked(&spec.service.name, spec.service.endpoint)
                .map_err(|_| Failure::new(FailureKind::InvalidInput, false))?,
            host: url::Host::Domain(spec.service.name.clone()),
            port: spec.service.port,
            roots: spec.service.roots.clone(),
            transport,
            quota: SocketQuota::checked(spec.limits.sockets)
                .map_err(|_| Failure::new(FailureKind::InvalidInput, false))?,
            whole: spec.limits.whole,
            connect: spec.limits.connect,
            attempts: spec.limits.attempts,
            input_bytes: spec.limits.input_bytes,
            method: spec.method,
            #[cfg(test)]
            driver_hold: None,
        };
        let origin = startup_origin(&profile)?;
        let authority = hyper::header::HeaderValue::from_str(if profile.port == 443 {
            &spec.service.name
        } else {
            origin.authority()
        })
        .map_err(|_| Failure::new(FailureKind::InvalidInput, false))?;
        fixed_headers
            .validate(&authority, &transport)
            .map_err(|_| Failure::new(FailureKind::InvalidInput, false))?;
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
