//! Public finite response construction. Context is a request-borrowed budget,
//! never an identity/lease/authorization proof. No implicit producer exists.
use super::response_budget::finalize::{CompletionOrigin, PreparedResponse, ResponseMeta};
use super::response_budget::wire::{FallbackLedger, FallbackLimits, FallbackStatus};
use super::response_budget::{Diagnostic, Limits, Produced, Producer};
use super::*;

#[derive(Debug)]
pub struct ResponseLimits {
    body_bytes: usize,
    user_header_bytes: usize,
    user_header_count: usize,
    managed_header_bytes: usize,
    managed_header_count: usize,
    serialization_writes: usize,
    live_responses: usize,
    retained_bytes: usize,
    fallback_responses: usize,
    fallback_retained_bytes: usize,
    fallback_managed_header_bytes: usize,
    fallback_managed_header_count: usize,
}
#[allow(clippy::too_many_arguments)]
pub fn response_limits(
    body_bytes: i64,
    user_header_bytes: i64,
    user_header_count: i64,
    managed_header_bytes: i64,
    managed_header_count: i64,
    serialization_writes: i64,
    live_responses: i64,
    retained_bytes: i64,
    fallback_responses: i64,
    fallback_retained_bytes: i64,
    fallback_managed_header_bytes: i64,
    fallback_managed_header_count: i64,
) -> Result<ResponseLimits, Error> {
    let checked = |n, label| positive_size(n, isize::MAX as usize, label);
    Ok(ResponseLimits {
        body_bytes: checked(body_bytes, "response body bytes")?,
        user_header_bytes: checked(user_header_bytes, "response user header bytes")?,
        user_header_count: checked(user_header_count, "response user header count")?,
        managed_header_bytes: checked(managed_header_bytes, "response managed header bytes")?,
        managed_header_count: checked(managed_header_count, "response managed header count")?,
        serialization_writes: checked(serialization_writes, "response serialization writes")?,
        live_responses: checked(live_responses, "live responses")?,
        retained_bytes: checked(retained_bytes, "response retained bytes")?,
        fallback_responses: checked(fallback_responses, "fallback responses")?,
        fallback_retained_bytes: checked(fallback_retained_bytes, "fallback retained bytes")?,
        fallback_managed_header_bytes: checked(
            fallback_managed_header_bytes,
            "fallback header bytes",
        )?,
        fallback_managed_header_count: checked(
            fallback_managed_header_count,
            "fallback header count",
        )?,
    })
}
impl ResponseLimits {
    fn ordinary(&self) -> Limits {
        Limits {
            body_bytes: self.body_bytes,
            user_header_bytes: self.user_header_bytes,
            user_header_count: self.user_header_count,
            managed_header_bytes: self.managed_header_bytes,
            managed_header_count: self.managed_header_count,
            serialization_writes: self.serialization_writes,
            live_responses: self.live_responses,
            retained_bytes: self.retained_bytes,
        }
    }
    fn fallback(&self) -> FallbackLimits {
        FallbackLimits {
            responses: self.fallback_responses,
            retained_bytes: self.fallback_retained_bytes,
            managed_header_bytes: self.fallback_managed_header_bytes,
            managed_header_count: self.fallback_managed_header_count,
        }
    }
}
pub fn with_response_limits(
    mut options: Options,
    limits: ResponseLimits,
) -> Result<Options, Error> {
    options.response_limits = Some(limits);
    validate_options(&options)?;
    Ok(options)
}
pub(super) fn validate_options(options: &Options) -> Result<(), Error> {
    if options.response_limits.is_some() {
        // Validation does not install a producer or supply missing settings.
        let _ = RuntimeResponses::checked(options, 0)?;
    }
    Ok(())
}
pub(super) struct RuntimeResponses {
    pub(super) producer: Arc<Producer>,
    pub(super) fallback: Arc<FallbackLedger>,
    pub(super) requests: Arc<Semaphore>,
}
impl fmt::Debug for RuntimeResponses {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RuntimeResponses { .. }")
    }
}
fn checked_add(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b)
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| Error::invalid("response metadata budget overflow"))
}
impl RuntimeResponses {
    pub(super) fn checked(options: &Options, catalog_bytes: usize) -> Result<Arc<Self>, Error> {
        let limits = options
            .response_limits
            .as_ref()
            .ok_or_else(|| Error::invalid("explicit response limits are required"))?;
        let owner_bytes = std::mem::size_of::<Request>()
            .checked_add(std::mem::size_of::<request_headers::ReceivedHeaders>())
            .and_then(|n| n.checked_add(4 * std::mem::size_of::<usize>()))
            .and_then(|n| n.checked_mul(options.requests))
            .ok_or_else(|| Error::invalid("request owner budget overflow"))?;
        let metadata = checked_add(
            catalog_bytes,
            checked_add(
                owner_bytes,
                checked_add(
                    std::mem::size_of::<Self>(),
                    checked_add(
                        std::mem::size_of::<Semaphore>(),
                        4 * std::mem::size_of::<usize>(),
                    )?,
                )?,
            )?,
        )?;
        let producer = Producer::checked_attached(limits.ordinary(), options.connections, metadata)
            .map_err(budget_error)?;
        let fallback = FallbackLedger::checked(limits.fallback(), options.connections)
            .map_err(budget_error)?;
        Ok(Arc::new(Self {
            producer: Arc::new(producer),
            fallback: Arc::new(fallback),
            requests: Arc::new(Semaphore::new(options.requests)),
        }))
    }
    pub(super) fn context(&self) -> ResponseContext {
        ResponseContext {
            profile: ResponseProfile::default(),
            producer: Arc::clone(&self.producer),
            fallback: Arc::clone(&self.fallback),
        }
    }
}
#[derive(Clone, Default)]
pub(super) struct ResponseProfile {
    pub(super) origin: Option<HeaderValue>,
    pub(super) credentials: bool,
    pub(super) dependencies: vary::Dependencies,
}
pub struct ResponseContext {
    pub(super) profile: ResponseProfile,
    pub(super) producer: Arc<Producer>,
    pub(super) fallback: Arc<FallbackLedger>,
}
impl ResponseContext {
    pub(super) fn prepare(
        &self,
        mut prepared: PreparedResponse,
    ) -> Result<PreparedResponse, Diagnostic> {
        if self.profile.origin.is_some() {
            prepared.cors(self.profile.origin.clone(), self.profile.credentials)?;
        }
        prepared.vary(self.profile.dependencies.core());
        prepared.preflight_terminal()?;
        Ok(prepared)
    }

    pub(super) fn framework(
        &self,
        kind: response_budget::finalize::FrameworkBody,
        cookie: Option<usize>,
    ) -> Result<response_budget::finalize::FrameworkTicket, response_budget::finalize::FrameworkError>
    {
        use response_budget::finalize::{FrameworkError, FrameworkTicket};
        FrameworkTicket::reserve(&self.producer, kind, cookie)
            .and_then(|ticket| ticket.cors(self.profile.origin.clone(), self.profile.credentials))
            .and_then(|ticket| ticket.dependencies(self.profile.dependencies.core()))
            .map_err(FrameworkError::Budget)
    }
    pub(super) fn share(&self) -> Self {
        Self {
            profile: self.profile.clone(),
            producer: Arc::clone(&self.producer),
            fallback: Arc::clone(&self.fallback),
        }
    }
}
impl fmt::Debug for ResponseContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ResponseContext { .. }")
    }
}
pub fn response_context(request: &Request) -> &ResponseContext {
    request.headers.response_context()
}
pub(super) fn budget_error(error: Diagnostic) -> Error {
    match error {
        Diagnostic::BodyLimit
        | Diagnostic::HeaderLimit
        | Diagnostic::WorkLimit
        | Diagnostic::InvalidLimits => Error::invalid(error.message()),
        Diagnostic::Capacity => Error {
            kind: ErrorKind::Busy,
            message: error.message().into(),
        },
        _ => Error::internal(error.message()),
    }
}
pub(super) fn refusal_status(status: Status, error: Diagnostic) -> Status {
    match error {
        Diagnostic::Fault | Diagnostic::InvalidLimits => Status::INTERNAL_SERVER_ERROR,
        _ if status.0.as_u16() >= 400 => status,
        _ => Status::SERVICE_UNAVAILABLE,
    }
}
pub struct Response {
    pub(super) session_seal: Option<crate::auth::session::ResponseSeal>,
    pub(super) status: Status,
    pub(super) produced: Produced,
    pub(super) body_kind: BodyKind,
    producer: Arc<Producer>,
    fallback: Arc<FallbackLedger>,
}
impl Response {
    pub(crate) fn has_session_seal(&self) -> bool {
        self.session_seal.is_some()
    }
    pub(crate) fn with_session_seal(mut self, seal: crate::auth::session::ResponseSeal) -> Self {
        self.session_seal = Some(seal);
        self
    }
    pub fn status(&self) -> Status {
        self.status
    }
    pub fn body(&self) -> &[u8] {
        self.produced.body()
    }
    pub(super) fn context(&self) -> ResponseContext {
        ResponseContext {
            profile: ResponseProfile::default(),
            producer: Arc::clone(&self.producer),
            fallback: Arc::clone(&self.fallback),
        }
    }
    pub(super) fn into_http(self, head: bool) -> HttpResponse {
        let context = self.context();
        self.into_http_origin(&context, head, CompletionOrigin::HandlerOk)
    }
    pub(super) fn into_replacement_http(
        self,
        context: &ResponseContext,
        head: bool,
    ) -> HttpResponse {
        self.into_http_origin(context, head, CompletionOrigin::Replacement)
    }
    fn into_http_origin(
        self,
        context: &ResponseContext,
        head: bool,
        origin: CompletionOrigin,
    ) -> HttpResponse {
        let status = self.status.0;
        let fallback = Arc::clone(&context.fallback);
        let prepared = PreparedResponse::new(
            &context.producer,
            self.produced,
            ResponseMeta {
                status,
                kind: self.body_kind,
                head,
                source: PolicySource::Public,
                origin,
            },
        )
        .and_then(|prepared| context.prepare(prepared))
        .and_then(PreparedResponse::finish);
        match prepared {
            Ok(wire) => HttpResponse::from_wire(status, wire, fallback),
            Err(error) => {
                HttpResponse::terminal(fallback, refusal_status(self.status, error), head, true)
            }
        }
    }
}
impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Response")
            .field("status", &self.status)
            .field("body_bytes", &self.body().len())
            .finish_non_exhaustive()
    }
}
fn representation(
    context: &ResponseContext,
    status: Status,
    kind: BodyKind,
    make: impl FnOnce(&Producer) -> Result<Produced, Diagnostic>,
) -> Result<Response, Error> {
    Ok(Response {
        session_seal: None,
        status,
        produced: make(&context.producer).map_err(budget_error)?,
        body_kind: kind,
        producer: Arc::clone(&context.producer),
        fallback: Arc::clone(&context.fallback),
    })
}
pub fn empty(context: &ResponseContext, status: Status) -> Result<Response, Error> {
    representation(context, status, BodyKind::Empty, Producer::empty)
}
pub fn text(context: &ResponseContext, status: Status, body: &str) -> Result<Response, Error> {
    representation(context, status, BodyKind::Text, |p| p.text(body))
}
pub fn bytes(context: &ResponseContext, status: Status, body: &[u8]) -> Result<Response, Error> {
    representation(context, status, BodyKind::Binary, |p| p.bytes(body))
}
pub fn json<T: Serialize + ?Sized>(
    context: &ResponseContext,
    status: Status,
    body: &T,
) -> Result<Response, Error> {
    representation(context, status, BodyKind::Json, |p| p.json(body))
}
pub fn html_response(
    context: &ResponseContext,
    status: Status,
    document: crate::html::HtmlDocument,
) -> Result<Response, Error> {
    representation(context, status, BodyKind::Html, |p| p.html(&document))
}
pub fn append_header(mut response: Response, name: &str, value: &[u8]) -> Result<Response, Error> {
    response
        .produced
        .append_user_header(name, value, response.body_kind == BodyKind::Html)
        .map_err(budget_error)?;
    Ok(response)
}
pub fn append_header_text(response: Response, name: &str, value: &str) -> Result<Response, Error> {
    append_header(response, name, value.as_bytes())
}

// Internal response remains a charged wrapper until the actual Hyper handoff.
// None is a final-close outcome, never an uncharged executable response.
pub(super) struct HttpResponse {
    status: StatusCode,
    headers: Option<response_budget::wire::ChargedNativeHeaders>,
    body: BufferedBody,
    fallback: Arc<FallbackLedger>,
    pub(super) source: PolicySource,
    pub(super) dependencies: vary::Dependencies,
    deletion: Option<HeaderValue>,
    // Set only by closed dispatcher/auth refusal paths, never inferred from a
    // user Response status or construction Context.
    runtime_refusal: Option<Status>,
    rate_reason: Option<FallbackStatus>,
    #[cfg(test)]
    allow_allocations: usize,
}
fn empty_headers() -> &'static HeaderMap {
    static EMPTY: std::sync::OnceLock<HeaderMap> = std::sync::OnceLock::new();
    EMPTY.get_or_init(HeaderMap::new)
}
pub(super) struct HeadersMut<'a>(Option<&'a mut response_budget::wire::ChargedNativeHeaders>);
impl std::ops::Deref for HeadersMut<'_> {
    type Target = HeaderMap;
    fn deref(&self) -> &HeaderMap {
        self.0
            .as_ref()
            .map_or(empty_headers(), |headers| headers.map())
    }
}
impl HeadersMut<'_> {
    pub(super) fn insert<K: axum::http::header::IntoHeaderName + AsRef<str>>(
        &mut self,
        name: K,
        value: HeaderValue,
    ) -> Option<HeaderValue> {
        self.0
            .as_deref_mut()
            .and_then(|headers| headers.insert(name, value))
    }
    pub(super) fn append<K: axum::http::header::IntoHeaderName + AsRef<str>>(
        &mut self,
        name: K,
        value: HeaderValue,
    ) -> bool {
        self.0
            .as_deref_mut()
            .is_some_and(|headers| headers.append(name, value))
    }
    pub(super) fn remove<K: axum::http::header::AsHeaderName>(
        &mut self,
        name: K,
    ) -> Option<HeaderValue> {
        self.0
            .as_deref_mut()
            .and_then(|headers| headers.remove(name))
    }
}
impl HttpResponse {
    pub(super) fn from_wire(
        status: StatusCode,
        wire: response_budget::wire::ChargedWire,
        fallback: Arc<FallbackLedger>,
    ) -> Self {
        let (headers, bytes) = wire.into_parts();
        Self {
            status,
            headers: Some(headers),
            body: BufferedBody {
                bytes,
                sending: None,
            },
            fallback,
            source: PolicySource::Public,
            dependencies: vary::Dependencies::default(),
            deletion: None,
            runtime_refusal: None,
            rate_reason: None,
            #[cfg(test)]
            allow_allocations: 0,
        }
    }
    pub(super) fn terminal(
        fallback: Arc<FallbackLedger>,
        status: Status,
        head: bool,
        close: bool,
    ) -> Self {
        Self::terminal_profile(fallback, status, PolicySource::Public, head, close)
    }
    pub(super) fn terminal_profile(
        fallback: Arc<FallbackLedger>,
        status: Status,
        source: PolicySource,
        head: bool,
        close: bool,
    ) -> Self {
        let reason = FallbackStatus::all()
            .into_iter()
            .find(|reason| reason.code() == status.0.as_u16())
            .unwrap_or(FallbackStatus::Internal);
        let intent = fallback
            .intent(reason)
            .context(source, head, response_budget::finalize::VarySet::EMPTY)
            .connection_close(close);
        match fallback.terminal(intent).into_response() {
            Some((code, wire)) => {
                let mut response = Self::from_wire(
                    StatusCode::from_u16(code).expect("closed fallback status"),
                    wire,
                    fallback,
                );
                response.source = source;
                response.dependencies = vary::Dependencies::source(source);
                response
            }
            None => Self {
                status: status.0,
                headers: None,
                body: BufferedBody {
                    bytes: Bytes::new(),
                    sending: None,
                },
                fallback,
                source: PolicySource::Public,
                dependencies: vary::Dependencies::default(),
                deletion: None,
                runtime_refusal: None,
                rate_reason: None,
                #[cfg(test)]
                allow_allocations: 0,
            },
        }
    }
    pub(super) fn rate_refusal(
        context: &ResponseContext,
        reason: FallbackStatus,
        source: PolicySource,
        head: bool,
    ) -> Self {
        let fallback = Arc::clone(&context.fallback);
        let status = StatusCode::from_u16(reason.code()).expect("closed rate refusal status");
        let intent = fallback
            .intent(reason)
            .context(source, head, context.profile.dependencies.core())
            .connection_close(true);
        let intent = if let Some(origin) = &context.profile.origin {
            intent.cors(origin.clone(), context.profile.credentials)
        } else {
            Ok(intent)
        };
        let mut response = match intent
            .ok()
            .and_then(|intent| fallback.terminal(intent).into_response())
        {
            Some((_, wire)) => Self::from_wire(status, wire, Arc::clone(&fallback)),
            None => Self {
                status,
                headers: None,
                body: BufferedBody {
                    bytes: Bytes::new(),
                    sending: None,
                },
                fallback,
                source,
                dependencies: context.profile.dependencies,
                deletion: None,
                runtime_refusal: None,
                rate_reason: None,
                #[cfg(test)]
                allow_allocations: 0,
            },
        };
        response.source = source;
        response.dependencies = context.profile.dependencies;
        response.rate_reason = Some(reason);
        response
    }
    pub(super) fn established_refusal(mut self) -> Self {
        if matches!(self.status.as_u16(), 401 | 403 | 408 | 413 | 504) {
            self.runtime_refusal = Some(Status(self.status));
        }
        self
    }
    pub(super) fn replacement(&self, status: Status, head: bool, close: bool) -> Self {
        let status = self.runtime_refusal.unwrap_or(status);
        let mut replacement = Self::terminal(Arc::clone(&self.fallback), status, head, close);
        replacement.runtime_refusal = self.runtime_refusal;
        replacement.rate_reason = self.rate_reason;
        replacement.source = self.source;
        replacement.dependencies = self.dependencies;
        replacement
    }
    pub(super) fn status(&self) -> StatusCode {
        self.status
    }
    pub(super) fn headers(&self) -> &HeaderMap {
        self.headers.as_ref().map_or(empty_headers(), |h| h.map())
    }
    pub(super) fn headers_mut(&mut self) -> HeadersMut<'_> {
        HeadersMut(self.headers.as_mut())
    }
    pub(super) fn body(&self) -> &BufferedBody {
        &self.body
    }
    pub(super) fn body_mut(&mut self) -> &mut BufferedBody {
        &mut self.body
    }
    // No method-name Vec or joined string exists until the complete serialized
    // Allow length fits this already-reserved charged native map. Repeated
    // borrowed scans preserve lexicographic ordering without an extra table.
    pub(super) fn allow_methods<'a>(
        &mut self,
        methods: impl Iterator<Item = &'a str> + Clone,
    ) -> Result<(), Diagnostic> {
        let include_head = methods.clone().any(|method| method == "GET")
            && !methods.clone().any(|method| method == "HEAD");
        let mut count = usize::from(include_head);
        let mut length: usize = if include_head { 4 } else { 0 };
        for method in methods.clone() {
            count = count.checked_add(1).ok_or(Diagnostic::HeaderLimit)?;
            length = length
                .checked_add(method.len())
                .ok_or(Diagnostic::HeaderLimit)?;
        }
        if count == 0 {
            return Err(Diagnostic::Fault);
        }
        length = count
            .checked_sub(1)
            .and_then(|n| n.checked_mul(2))
            .and_then(|n| n.checked_add(length))
            .ok_or(Diagnostic::HeaderLimit)?;
        self.headers
            .as_ref()
            .ok_or(Diagnostic::Capacity)?
            .preflight_value("allow", length, true)?;
        #[cfg(test)]
        {
            self.allow_allocations += 1;
        }
        let mut value = String::new();
        value
            .try_reserve_exact(length)
            .map_err(|_| Diagnostic::Allocation)?;
        if value.capacity() > length {
            return Err(Diagnostic::Allocation);
        }
        let mut previous = None;
        for index in 0..count {
            let next = methods
                .clone()
                .chain(include_head.then_some("HEAD"))
                .filter(|method| previous.map_or(true, |old| *method > old))
                .min()
                .ok_or(Diagnostic::Fault)?;
            if index != 0 {
                value.push_str(", ");
            }
            value.push_str(next);
            previous = Some(next);
        }
        if value.len() != length {
            return Err(Diagnostic::Fault);
        }
        let value = HeaderValue::from_str(&value).map_err(|_| Diagnostic::HeaderLimit)?;
        let headers = self.headers.as_mut().ok_or(Diagnostic::Capacity)?;
        headers.insert(names::ALLOW, value);
        headers.fault().map_or(Ok(()), Err)
    }
    pub(super) fn close_before_native(mut self) -> Self {
        // No native handoff occurred. Dispose both charged owners, then signal
        // final close through the existing no-response outcome.
        self.headers = None;
        self.body.bytes = Bytes::new();
        self
    }
    pub(super) fn has_consumed_deletion(&self) -> bool {
        self.deletion.is_some()
    }
    pub(super) fn remember_deletion(&mut self, cookie: HeaderValue) {
        self.deletion = Some(cookie);
    }
    pub(super) fn consumed_deletion(&mut self, cookie: HeaderValue) {
        if self.headers.is_none() {
            return;
        }
        self.deletion = Some(cookie.clone());
        self.headers_mut().append(names::SET_COOKIE, cookie);
    }
    pub(super) fn transfer_native(
        mut self,
        ledger: &response_send::SendLedger,
        head: bool,
    ) -> Result<axum::http::Response<BufferedBody>, response_send::SendFault> {
        let Some(mut headers) = self.headers else {
            return Err(response_send::SendFault::Exhausted);
        };
        if let Some(error) = headers.fault() {
            // A sealed prior runtime refusal wins. Otherwise apply the same
            // classifier as prepared refusal: internal faults are 500, while
            // resource errors retain an existing ordinary denial status.
            let status = self
                .runtime_refusal
                .unwrap_or_else(|| refusal_status(Status(self.status), error));
            let reason = if status.0 == StatusCode::TOO_MANY_REQUESTS {
                self.rate_reason.unwrap_or(FallbackStatus::Internal)
            } else {
                FallbackStatus::all()
                    .into_iter()
                    .find(|r| r.code() == status.0.as_u16())
                    .unwrap_or(FallbackStatus::Internal)
            };
            let mut intent = self
                .fallback
                .intent(reason)
                .context(self.source, head, self.dependencies.core())
                .connection_close(true);
            if let Some(origin) = headers.remove(names::ACCESS_CONTROL_ALLOW_ORIGIN) {
                intent = intent
                    .cors(
                        origin,
                        headers
                            .map()
                            .get(names::ACCESS_CONTROL_ALLOW_CREDENTIALS)
                            .is_some(),
                    )
                    .map_err(|_| response_send::SendFault::Exhausted)?;
            }
            if let Some(allow) = headers.remove(names::ALLOW) {
                intent = intent
                    .allow(allow)
                    .map_err(|_| response_send::SendFault::Exhausted)?;
            }
            if let Some(deletion) = self.deletion.take() {
                intent = intent
                    .consumed_deletion(deletion)
                    .map_err(|_| response_send::SendFault::Exhausted)?;
            }
            let Some((code, wire)) = self.fallback.terminal(intent).into_response() else {
                return Err(response_send::SendFault::Exhausted);
            };
            // Original headers/body remain charged through fallback allocation.
            return wire.transfer_native(
                ledger,
                StatusCode::from_u16(code).map_err(|_| response_send::SendFault::Closed)?,
            );
        }
        let mut wire = response_budget::wire::ChargedWire::from_parts(headers, self.body.bytes);
        if head {
            wire.suppress_body_for_head();
        }
        wire.transfer_native(ledger, self.status)
    }
}

#[cfg(test)]
#[path = "response_public_tests.rs"]
pub(super) mod tests;
