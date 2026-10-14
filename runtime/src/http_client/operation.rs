//! Adopted A1–A5 operation values. Host catalog/profile construction stays native.
use super::admission::AdmissionError;
use super::parent_drain::RunnerOwnerHandle;
use super::tracked_dns::{resolve, FamilyOutcome, ResolverStartup};
use super::*;
use crate::http_client::address::{self, InternalPolicy, ServiceIdentity};
use address::q1_dial::{
    AttemptControl, AttemptOrder, DialPlan, OperationBrand, PreferredFamily, SuccessfulLookup,
};
use address::socket_pin::q1_native::{connect_plan, FailoverError};
use address::socket_pin::tls_http1::{
    establish, BoundedResponse, TlsProfile, TransportError, TransportLimits, WireRequest,
};
use address::socket_pin::{DialControl, SocketQuota};
use hyper::{HeaderMap, Method, Uri};
use std::{
    marker::PhantomData,
    num::NonZeroU64,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureKind {
    InvalidInput,
    Security,
    DestinationDenied,
    Capacity,
    Closing,
    Deadline,
    Dns,
    Tcp,
    Tls,
    Protocol,
    ResponseLimit,
    UnsupportedEncoding,
    Unavailable,
    Internal,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    RequestNotStarted,
    Unknown,
}
pub struct Failure {
    kind: FailureKind,
    auth: Option<crate::auth::FailureKind>,
    outcome: Outcome,
}
impl std::fmt::Debug for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Failure")
            .field("kind", &self.kind)
            .field("outcome", &self.outcome)
            .finish()
    }
}
impl Failure {
    pub fn invalid_input() -> Self {
        Self::new(FailureKind::InvalidInput, false)
    }
    pub fn unavailable() -> Self {
        Self::new(FailureKind::Unavailable, false)
    }
    fn new(kind: FailureKind, submitted: bool) -> Self {
        Self {
            kind,
            auth: None,
            outcome: if submitted {
                Outcome::Unknown
            } else {
                Outcome::RequestNotStarted
            },
        }
    }
}
pub fn kind(failure: &Failure) -> FailureKind {
    failure.kind
}
pub fn auth_kind(failure: &Failure) -> Option<crate::auth::FailureKind> {
    failure.auth
}
pub fn outcome(failure: &Failure) -> Outcome {
    failure.outcome
}
pub fn message(failure: &Failure) -> &str {
    match failure.kind {
        FailureKind::InvalidInput => "invalid input",
        FailureKind::Security => "authorization refused",
        FailureKind::DestinationDenied => "destination refused",
        FailureKind::Capacity => "capacity exhausted",
        FailureKind::Closing => "client closing",
        FailureKind::Deadline => "deadline expired",
        FailureKind::Dns => "resolution failed",
        FailureKind::Tcp => "connection failed",
        FailureKind::Tls => "TLS failed",
        FailureKind::Protocol => "invalid response",
        FailureKind::ResponseLimit => "response limit exceeded",
        FailureKind::UnsupportedEncoding => "unsupported encoding",
        FailureKind::Unavailable => "service unavailable",
        FailureKind::Internal => "client fault",
    }
}
pub struct Response {
    status: hyper::StatusCode,
    initial_headers: HeaderMap,
    body: Vec<u8>,
}
impl From<BoundedResponse> for Response {
    fn from(value: BoundedResponse) -> Self {
        Self {
            status: value.status,
            initial_headers: value.headers,
            body: value.body,
        }
    }
}
pub fn status(response: &Response) -> i64 {
    i64::from(response.status.as_u16())
}
pub fn body(response: &Response) -> &[u8] {
    &response.body
}
pub fn take_body(response: Response) -> Vec<u8> {
    response.body
}
pub fn headers<'r>(response: &'r Response, name: &str) -> Result<Vec<&'r [u8]>, crate::Error> {
    let name = hyper::header::HeaderName::from_bytes(name.as_bytes())
        .map_err(|_| crate::Error::invalid("invalid header name"))?;
    let values = response.initial_headers.get_all(name);
    let mut out = Vec::new();
    out.try_reserve_exact(values.iter().count())
        .map_err(|_| crate::Error::internal("header allocation failed"))?;
    out.extend(values.iter().map(|value| value.as_bytes()));
    Ok(out)
}

// Numerical values below are explicit private test configuration, never defaults.
pub(super) struct NativeProfile {
    pub(super) identity: ServiceIdentity,
    pub(super) policy: Arc<InternalPolicy>,
    pub(super) resolver: ResolverStartup,
    pub(super) host: url::Host<String>,
    pub(super) port: u16,
    pub(super) roots: rustls::RootCertStore,
    pub(super) transport: TransportLimits,
    pub(super) quota: SocketQuota,
    pub(super) whole: Duration,
    pub(super) connect: Duration,
    pub(super) attempts: usize,
    pub(super) input_bytes: usize,
    pub(super) method: Method,
    #[cfg(test)]
    pub(super) driver_hold: Option<Arc<Notify>>,
}
pub struct InputLimits {
    max_bytes: usize,
}
impl InputLimits {
    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }
}
pub struct RequestLimits {
    origin: url::Url,
    max_body_bytes: usize,
    max_target_bytes: usize,
}
pub struct FrozenInput(Vec<u8>);
impl FrozenInput {
    pub fn checked(bytes: Vec<u8>, limits: &InputLimits) -> Result<Self, Failure> {
        if bytes.len() > limits.max_bytes {
            return Err(Failure::new(FailureKind::InvalidInput, false));
        }
        let mut compact = Vec::new();
        compact
            .try_reserve_exact(bytes.len())
            .map_err(|_| Failure::new(FailureKind::Unavailable, false))?;
        compact.extend_from_slice(&bytes);
        Ok(Self(compact))
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}
pub struct BoundValues {
    subject: i64,
    target: i64,
}
impl BoundValues {
    pub fn subject(&self) -> i64 {
        self.subject
    }
    pub fn target(&self) -> i64 {
        self.target
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum RequestBinding {
    Protected { subject: i64, target: i64 },
    Public,
}
pub struct PreparedRequest {
    uri: Uri,
    body: Vec<u8>,
    binding: RequestBinding,
}
impl PreparedRequest {
    pub fn protected(
        bound: BoundValues,
        segments: &[&str],
        query: &[(&str, &str)],
        body: Vec<u8>,
        limits: &RequestLimits,
    ) -> Result<Self, Failure> {
        Self::structured(
            segments,
            query,
            body,
            limits,
            RequestBinding::Protected {
                subject: bound.subject,
                target: bound.target,
            },
        )
    }
    pub fn public(
        segments: &[&str],
        query: &[(&str, &str)],
        body: Vec<u8>,
        limits: &RequestLimits,
    ) -> Result<Self, Failure> {
        Self::structured(segments, query, body, limits, RequestBinding::Public)
    }
    fn structured(
        segments: &[&str],
        query: &[(&str, &str)],
        body: Vec<u8>,
        limits: &RequestLimits,
        binding: RequestBinding,
    ) -> Result<Self, Failure> {
        if body.len() > limits.max_body_bytes || segments.iter().any(|s| *s == "." || *s == "..") {
            return Err(Failure::new(FailureKind::InvalidInput, false));
        }
        // The only URL parse was startup; per-operation construction clones that
        // frozen origin and uses URL's segment/query encoder, never input as URL.
        let mut url = limits.origin.clone();
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|_| Failure::new(FailureKind::Internal, false))?;
            path.clear();
            path.extend(segments.iter().copied());
        }
        if !query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            pairs.clear();
            pairs.extend_pairs(query.iter().copied());
        }
        let target = match url.query() {
            Some(query) => format!("{}?{}", url.path(), query),
            None => url.path().to_owned(),
        };
        if target.len() > limits.max_target_bytes {
            return Err(Failure::new(FailureKind::InvalidInput, false));
        }
        let uri = target
            .parse()
            .map_err(|_| Failure::new(FailureKind::InvalidInput, false))?;
        Ok(Self { uri, body, binding })
    }
}
struct Core {
    sender: ClientSender,
    runner: Arc<RunnerOwnerHandle>,
    profile: NativeProfile,
    input_limits: InputLimits,
    request_limits: RequestLimits,
    ready: Arc<AtomicBool>,
    operation_id: u64,
    fixed_headers: address::socket_pin::tls_http1::FixedHeaders,
}
pub struct ProtectedOperation<C, P, I> {
    core: Arc<Core>,
    state: Arc<C>,
    freeze: fn(I, &InputLimits) -> Result<FrozenInput, Failure>,
    prepare: fn(&C, BoundValues, FrozenInput, &RequestLimits) -> Result<PreparedRequest, Failure>,
    permission: PhantomData<fn() -> P>,
}
pub struct PublicOperation<C, I> {
    core: Arc<Core>,
    state: Arc<C>,
    freeze: fn(I, &InputLimits) -> Result<FrozenInput, Failure>,
    prepare: fn(&C, FrozenInput, &RequestLimits) -> Result<PreparedRequest, Failure>,
}
impl<C, P, I> std::fmt::Debug for ProtectedOperation<C, P, I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProtectedOperation")
    }
}
impl<C, I> std::fmt::Debug for PublicOperation<C, I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PublicOperation")
    }
}

fn ledger_failure(error: LedgerError, submitted: bool) -> Failure {
    let kind = match error {
        LedgerError::Capacity => FailureKind::Capacity,
        LedgerError::Closing | LedgerError::PhaseClosed => FailureKind::Closing,
        LedgerError::Expired => FailureKind::Deadline,
        LedgerError::Allocation | LedgerError::NoRuntime => FailureKind::Unavailable,
        _ => FailureKind::Internal,
    };
    Failure::new(kind, submitted)
}
fn admission_failure(error: AdmissionError) -> Failure {
    match error {
        AdmissionError::Ledger(error) => ledger_failure(error, false),
        AdmissionError::Auth(error) => Failure {
            kind: FailureKind::Security,
            auth: Some(error.kind()),
            outcome: Outcome::RequestNotStarted,
        },
    }
}
fn transport_failure(error: TransportError, submitted: bool) -> Failure {
    let kind = match error {
        TransportError::Tls => FailureKind::Tls,
        TransportError::Deadline => FailureKind::Deadline,
        TransportError::HeaderLimit | TransportError::BodyLimit => FailureKind::ResponseLimit,
        TransportError::UnsupportedEncoding => FailureKind::UnsupportedEncoding,
        TransportError::Capacity => FailureKind::Unavailable,
        TransportError::Socket(address::socket_pin::SocketError::Closing) => FailureKind::Closing,
        TransportError::Socket(address::socket_pin::SocketError::Faulted) => FailureKind::Internal,
        TransportError::Socket(address::socket_pin::SocketError::Deadline) => FailureKind::Deadline,
        _ => FailureKind::Protocol,
    };
    Failure::new(kind, submitted)
}
fn connect_failure(error: FailoverError) -> Failure {
    use address::q1_dial::PlanStop;
    use address::socket_pin::SocketError;
    let kind = match error {
        FailoverError::Socket(SocketError::Capacity) => FailureKind::Capacity,
        FailoverError::Socket(SocketError::Closing)
        | FailoverError::Plan(PlanStop::Closing | PlanStop::Cancelled) => FailureKind::Closing,
        FailoverError::Socket(SocketError::Deadline)
        | FailoverError::Plan(
            PlanStop::OperationExpired
            | PlanStop::ConnectExpired
            | PlanStop::ResolutionExpired
            | PlanStop::SlotExpired,
        ) => FailureKind::Deadline,
        FailoverError::Socket(SocketError::Address(_) | SocketError::PeerMismatch) => {
            FailureKind::DestinationDenied
        }
        FailoverError::Socket(SocketError::Faulted | SocketError::InvalidLimit)
        | FailoverError::Plan(
            PlanStop::Fault(_)
            | PlanStop::CleanupIncomplete
            | PlanStop::InvalidState
            | PlanStop::Finished,
        ) => FailureKind::Internal,
        _ => FailureKind::Tcp,
    };
    Failure::new(kind, false)
}

async fn native_request(
    core: &Core,
    key: &JobKey,
    end: Instant,
    request: PreparedRequest,
    submitted: Arc<AtomicBool>,
    binding: RequestBinding,
) -> Result<Response, Failure> {
    let profile = &core.profile;
    if request.binding != binding {
        return Err(Failure::new(FailureKind::InvalidInput, false));
    }
    let wire = WireRequest::checked_with_headers(
        profile.method.clone(),
        request.uri,
        request.body,
        &profile.transport,
        core.fixed_headers.clone(),
    )
    .map_err(|_| Failure::new(FailureKind::InvalidInput, false))?;
    core.runner
        .phase(key, || ())
        .map_err(|error| ledger_failure(error, false))?;
    let drained = resolve(&core.runner, key, &profile.resolver, end)
        .await
        .map_err(|_| Failure::new(FailureKind::Dns, false))?;
    let (_, a, aaaa, resolve_end) = drained.families();
    let mut ipv4 = Vec::new();
    let mut ipv6 = Vec::new();
    let mut ttls = Vec::new();
    let mut until: Option<Instant> = None;
    for family in [a, aaaa] {
        if let FamilyOutcome::Answers {
            addresses,
            lookup,
            positive_until,
        } = family
        {
            until = Some(until.map_or(*positive_until, |old| old.min(*positive_until)));
            for address in addresses {
                match address {
                    std::net::IpAddr::V4(value) => ipv4.push(*value),
                    std::net::IpAddr::V6(value) => ipv6.push(*value),
                }
            }
            for record in lookup.answers() {
                // Hickory already rejects unrelated answers; retain every chain TTL.
                ttls.push(record.ttl);
            }
        }
    }
    let operation = OperationBrand::from_admitted_operation(
        NonZeroU64::new(key.id).ok_or_else(|| Failure::new(FailureKind::Internal, false))?,
    );
    let resolution = SuccessfulLookup::from_provider_results(
        operation,
        Ok(ipv4),
        Ok(ipv6),
        ttls,
        until,
        resolve_end,
        end,
    )
    .and_then(|lookup| lookup.check(&profile.policy, profile.identity, Instant::now()))
    .map_err(|_| Failure::new(FailureKind::DestinationDenied, false))?;
    let plan = DialPlan::checked(
        resolution,
        AttemptOrder::Interleave {
            preferred: PreferredFamily::V4,
        },
        profile.attempts,
        profile.connect,
        Instant::now(),
    )
    .map_err(|_| Failure::new(FailureKind::Tcp, false))?;
    let control = Arc::new(DialControl::for_operation(
        NativeGate::for_job(&core.runner, key).map_err(|error| ledger_failure(error, false))?,
    ));
    let mut stop = StopTransport(Some(Arc::clone(&control)));
    let (socket, mut plan) = match connect_plan(plan, &control, &profile.quota).await {
        Ok(connected) => connected,
        Err(error) => {
            let failure = connect_failure(error);
            if failure.kind == FailureKind::Internal {
                let mut owner = core
                    .runner
                    .owner
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                owner.latch_fault(Fault::Invariant);
                owner.close();
            }
            return Err(failure);
        }
    };
    plan.start_tls(Instant::now(), &AttemptControl::default())
        .map_err(|_| Failure::new(FailureKind::Deadline, false))?;
    let tls = TlsProfile::checked(
        profile.identity,
        profile.host.clone(),
        profile.port,
        profile.roots.clone(),
        end,
        end,
        profile.transport,
    )
    .map_err(|_| Failure::new(FailureKind::InvalidInput, false))?;
    let parts = establish(socket, &tls, Arc::clone(&control))
        .await
        .map_err(|error| transport_failure(error, false))?;
    plan.finish_tls(Instant::now(), &AttemptControl::default())
        .map_err(|_| Failure::new(FailureKind::Deadline, false))?;
    #[cfg(test)]
    let driver_hold = profile.driver_hold.clone();
    core.runner
        .spawn(key, TaskKind::HttpDriver, async move {
            let _ = parts.driver.await;
            #[cfg(test)]
            if let Some(hold) = driver_hold {
                hold.notified().await;
            }
        })
        .map_err(|(error, driver)| {
            drop(driver);
            ledger_failure(error, false)
        })?;
    let ticket = plan
        .take_http_enqueue_ticket(Instant::now(), &AttemptControl::default())
        .map_err(|_| Failure::new(FailureKind::Deadline, false))?;
    let response = parts
        .client
        .send_once_observed(wire, Arc::clone(&submitted))
        .await;
    let started = submitted.load(Ordering::Acquire);
    plan.finish_http(ticket)
        .map_err(|_| Failure::new(FailureKind::Internal, started))?;
    if response.is_ok() {
        stop.0 = None;
    }
    response
        .map(Response::from)
        .map_err(|error| transport_failure(error, started))
}
struct StopTransport(Option<Arc<DialControl>>);
impl Drop for StopTransport {
    fn drop(&mut self) {
        if let Some(control) = &self.0 {
            control.close();
        }
    }
}

struct CancelOnDrop {
    key: JobKey,
    shared: Arc<Shared>,
    armed: bool,
}
// Not part of public Response: the charge exists only while the root/channel/
// execute future owns the provisional result. Envelope field order disposes
// owned response bytes before the ticket's lock-free release on receiver Drop.
struct PublicationTicket {
    key: JobKey,
    shared: Arc<Shared>,
    armed: bool,
}
impl PublicationTicket {
    fn new(key: &JobKey, core: &Core) -> Self {
        Self {
            key: key.clone(),
            shared: Arc::clone(&core.sender.shared),
            armed: true,
        }
    }
    fn acknowledge(&mut self) {
        if self.armed {
            self.key
                .seal
                .publication_pending
                .store(false, Ordering::Release);
            self.shared.stop_notify.notify_waiters();
            self.armed = false;
        }
    }
}
impl Drop for PublicationTicket {
    fn drop(&mut self) {
        self.acknowledge();
    }
}
struct CompletedResult {
    value: Result<Response, Failure>,
    ticket: PublicationTicket,
}
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if self.armed {
            self.key.seal.caller_cancel.store(true, Ordering::Release);
            self.shared.stop_notify.notify_waiters();
        }
    }
}

fn startup_origin(profile: &NativeProfile) -> Result<url::Url, Failure> {
    let name = match &profile.host {
        url::Host::Domain(name) => name,
        _ => return Err(Failure::new(FailureKind::InvalidInput, false)),
    };
    url::Url::parse(&format!("https://{name}:{}/", profile.port))
        .map_err(|_| Failure::new(FailureKind::InvalidInput, false))
}

// Minimal host phase. Registration retains a parent-owned monitor; the host
// keeps ParentDrain and actually awaits shutdown. No Nagi owner API is added.
#[cfg(test)]
pub(super) fn register_private_protected<C: Send + Sync + 'static, P, I>(
    parent: &mut ParentDrain,
    limits: Limits,
    profile: NativeProfile,
    state: Arc<C>,
    freeze: fn(I, &InputLimits) -> Result<FrozenInput, Failure>,
    prepare: fn(&C, BoundValues, FrozenInput, &RequestLimits) -> Result<PreparedRequest, Failure>,
) -> Result<ProtectedOperation<C, P, I>, Failure> {
    if profile.whole.is_zero()
        || profile.connect.is_zero()
        || profile.input_bytes == 0
        || profile.attempts == 0
    {
        return Err(Failure::new(FailureKind::InvalidInput, false));
    }
    let (id, sender) = parent
        .register(limits, profile.input_bytes)
        .map_err(|_| Failure::new(FailureKind::Unavailable, false))?;
    let runner = parent
        .runner_owner(id)
        .map_err(|_| Failure::new(FailureKind::Internal, false))?;
    let origin = startup_origin(&profile)?;
    let input_limits = InputLimits {
        max_bytes: profile.input_bytes,
    };
    let request_limits = RequestLimits {
        origin,
        max_body_bytes: profile.transport.request_limit(),
        max_target_bytes: profile.transport.path_limit(),
    };
    Ok(ProtectedOperation {
        core: Arc::new(Core {
            sender,
            runner: Arc::new(runner),
            profile,
            input_limits,
            request_limits,
            ready: Arc::new(AtomicBool::new(true)),
            operation_id: 1,
            fixed_headers: address::socket_pin::tls_http1::FixedHeaders::empty(),
        }),
        state,
        freeze,
        prepare,
        permission: PhantomData,
    })
}

pub async fn execute<C: Send + Sync + 'static, P, I>(
    operation: &ProtectedOperation<C, P, I>,
    grant: crate::auth::Grant<P>,
    input: I,
) -> Result<Response, Failure> {
    let end = Instant::now()
        .checked_add(operation.core.profile.whole)
        .ok_or_else(|| Failure::new(FailureKind::InvalidInput, false))?;
    if !operation.core.ready.load(Ordering::Acquire) {
        return Err(Failure::new(FailureKind::Unavailable, false));
    }
    let frozen = (operation.freeze)(input, &operation.core.input_limits)?;
    let key = operation
        .core
        .sender
        .submit_operation(
            grant,
            frozen.0,
            end,
            RegisteredOperation {
                service: operation.core.profile.identity,
                operation: operation.core.operation_id,
                explicit_public: false,
            },
        )
        .await
        .map_err(admission_failure)?;
    execute_admitted(
        Arc::clone(&operation.core),
        Arc::clone(&operation.state),
        Prepare::Protected(operation.prepare),
        key,
    )
    .await
}

enum Prepare<C> {
    Protected(fn(&C, BoundValues, FrozenInput, &RequestLimits) -> Result<PreparedRequest, Failure>),
    Public(fn(&C, FrozenInput, &RequestLimits) -> Result<PreparedRequest, Failure>),
}

pub async fn execute_public<C: Send + Sync + 'static, I>(
    operation: &PublicOperation<C, I>,
    input: I,
) -> Result<Response, Failure> {
    let end = Instant::now()
        .checked_add(operation.core.profile.whole)
        .ok_or_else(|| Failure::new(FailureKind::InvalidInput, false))?;
    if !operation.core.ready.load(Ordering::Acquire) {
        return Err(Failure::new(FailureKind::Unavailable, false));
    }
    let frozen = (operation.freeze)(input, &operation.core.input_limits)?;
    let key = operation
        .core
        .sender
        .submit_public_operation(
            frozen.0,
            end,
            RegisteredOperation {
                service: operation.core.profile.identity,
                operation: operation.core.operation_id,
                explicit_public: true,
            },
        )
        .await
        .map_err(|error| ledger_failure(error, false))?;
    execute_admitted(
        Arc::clone(&operation.core),
        Arc::clone(&operation.state),
        Prepare::Public(operation.prepare),
        key,
    )
    .await
}
async fn execute_admitted<C: Send + Sync + 'static>(
    operation_core: Arc<Core>,
    state: Arc<C>,
    prepare: Prepare<C>,
    key: JobKey,
) -> Result<Response, Failure> {
    let end = *key
        .seal
        .admitted_end
        .get()
        .expect("accepted bound command deadline");
    let mut cancel = CancelOnDrop {
        key: key.clone(),
        shared: Arc::clone(&operation_core.sender.shared),
        armed: true,
    };
    let submitted = Arc::new(AtomicBool::new(false));
    let ticket = PublicationTicket::new(&key, &operation_core);
    let expected_public = matches!(&prepare, Prepare::Public(_));
    let expected = RegisteredOperation {
        service: operation_core.profile.identity,
        operation: operation_core.operation_id,
        explicit_public: expected_public,
    };
    if key.seal.operation.get() != Some(&expected) {
        {
            let mut owner = operation_core
                .runner
                .owner
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            owner.latch_fault(Fault::Invariant);
            owner.close();
        }
        let result = publish_joined(
            &operation_core,
            &key,
            end,
            &submitted,
            Ok(CompletedResult {
                value: Err(Failure::new(FailureKind::Internal, false)),
                ticket,
            }),
        )
        .await;
        cancel.armed = false;
        return result;
    }
    let mut command = match operation_core.runner.take_command_for(key.id) {
        Ok(Some(command)) => command,
        result => {
            let error = match result {
                Err(error) => error,
                _ => LedgerError::Closing,
            };
            let _ = operation_core.runner.cancel(&key);
            let result = publish_joined(
                &operation_core,
                &key,
                end,
                &submitted,
                Ok(CompletedResult {
                    value: Err(ledger_failure(error, false)),
                    ticket,
                }),
            )
            .await;
            cancel.armed = false;
            return result;
        }
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    let core = Arc::clone(&operation_core);
    let binding = if command.is_public() {
        RequestBinding::Public
    } else {
        RequestBinding::Protected {
            subject: command.subject,
            target: command.resource,
        }
    };
    // The already compact command allocation moves into the root; no second
    // prepare callback or proof/permit is stored or handed to user code.
    let frozen = FrozenInput(command.take_payload());
    let job_key = key.clone();
    let root_submitted = Arc::clone(&submitted);
    let future = async move {
        let prepared = match (prepare, binding) {
            (Prepare::Protected(callback), RequestBinding::Protected { subject, target }) => {
                callback(
                    &state,
                    BoundValues { subject, target },
                    frozen,
                    &core.request_limits,
                )
            }
            (Prepare::Public(callback), RequestBinding::Public) => {
                callback(&state, frozen, &core.request_limits)
            }
            _ => Err(Failure::new(FailureKind::Internal, false)),
        };
        let result = match prepared {
            Err(error) => Err(error),
            Ok(request) => {
                native_request(&core, &job_key, end, request, root_submitted, binding).await
            }
        };
        let _ = tx.send(CompletedResult {
            value: result,
            ticket,
        });
    };
    let launched = operation_core.runner.spawn_root(&mut command, future);
    drop(command);
    let result = match launched {
        Ok(()) => rx.await,
        Err((error, future)) => {
            drop(future);
            Ok(CompletedResult {
                value: Err(ledger_failure(error, false)),
                ticket: PublicationTicket::new(&key, &operation_core),
            })
        }
    };
    let result = publish_joined(&operation_core, &key, end, &submitted, result).await;
    cancel.armed = false;
    result
}

async fn publish_joined(
    core: &Core,
    key: &JobKey,
    end: Instant,
    submitted: &AtomicBool,
    result: Result<CompletedResult, tokio::sync::oneshot::error::RecvError>,
) -> Result<Response, Failure> {
    // A result message is not a join. The retained monitor is the sole writer
    // of retirement, after every actual join and command/raw owner disposal.
    let mut events = core.sender.shared.events_tx.subscribe();
    loop {
        if key.seal.cleanup_complete.load(Ordering::Acquire) {
            break;
        }
        if events.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
    // The same short gate as admission/close serializes the final publication.
    // It is deliberately after cleanup even on deadline/cancellation/failure.
    let mut owner = core
        .runner
        .owner
        .lock()
        .map_err(|_| Failure::new(FailureKind::Internal, submitted.load(Ordering::Acquire)))?;
    let started = submitted.load(Ordering::Acquire);
    let fault = owner.fault();
    let closing = owner.is_closing() || key.seal.caller_cancel.load(Ordering::Acquire);
    // An arbitrary root/adapter panic remains a task fault after real cleanup;
    // it is never flattened into a business Result::Err(INTERNAL).
    if fault == Some(Fault::TaskPanic) {
        drop(owner);
        panic!("HTTP operation task fault after retained cleanup");
    }
    if fault.is_some() {
        drop(result);
        owner.release_finished_jobs();
        return Err(Failure::new(FailureKind::Internal, started));
    }
    if Instant::now() >= end {
        drop(result);
        owner.release_finished_jobs();
        return Err(Failure::new(FailureKind::Deadline, started));
    }
    if closing {
        drop(result);
        owner.release_finished_jobs();
        return Err(Failure::new(FailureKind::Closing, started));
    }
    match result {
        Ok(CompletedResult { value, mut ticket }) => {
            ticket.acknowledge();
            owner.release_finished_jobs();
            value.map_err(|mut failure| {
                if started {
                    failure.outcome = Outcome::Unknown;
                }
                failure
            })
        }
        Err(_) => {
            drop(owner);
            panic!("HTTP operation root disappeared without a classified result");
        }
    }
}

#[cfg(test)]
#[path = "operation_tests.rs"]
mod tests;

#[path = "startup.rs"]
pub(in crate::http_client) mod startup;
