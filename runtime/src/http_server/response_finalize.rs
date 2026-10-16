//! Private finite finalizer core; not connected to the public Response/dispatch.
//! All native header mutation is append-only after a single typed plan is ready.
use super::wire::{ChargedWire, FallbackStatus, ManagedHeader};
use super::*;
use crate::http_server::{BodyKind, PolicySource, CONTENT_SECURITY_POLICY};
use axum::http::{HeaderValue, StatusCode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::http_server) enum CompletionOrigin {
    HandlerOk,
    Replacement,
}
#[derive(Clone, Copy)]
pub(in crate::http_server) struct VarySet(u8);
impl VarySet {
    pub(in crate::http_server) const EMPTY: Self = Self(0);
    pub(in crate::http_server) const ORIGIN: Self = Self(1 << 5);
    pub(in crate::http_server) const COOKIE: Self = Self(1 << 4);
    pub(in crate::http_server) const AUTH: Self = Self((1 << 2) | (1 << 4));
    pub(in crate::http_server) const LOGIN: Self = Self(Self::AUTH.0 | Self::ORIGIN.0);
    pub(in crate::http_server) const GUARDED: Self = Self(Self::LOGIN.0 | (1 << 6));
    pub(in crate::http_server) const BROWSER: Self = Self(Self::GUARDED.0 | (1 << 3));
    pub(in crate::http_server) const PREFLIGHT: Self = Self((1 << 0) | (1 << 1) | Self::ORIGIN.0);
    pub(in crate::http_server) fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    fn source(source: PolicySource) -> Self {
        match source {
            PolicySource::Public => Self::EMPTY,
            PolicySource::Bearer | PolicySource::Session => Self::AUTH,
            PolicySource::Prelogin => Self::LOGIN,
            PolicySource::BrowserLogin => Self::BROWSER,
        }
    }
    fn encode(self, buffer: &mut [u8]) -> usize {
        let names = [
            "Access-Control-Request-Headers",
            "Access-Control-Request-Method",
            "Authorization",
            "Content-Type",
            "Cookie",
            "Origin",
            "X-CSRF-Token",
        ];
        let mut used = 0;
        for (index, name) in names.iter().enumerate() {
            if self.0 & (1 << index) != 0 {
                if used != 0 {
                    buffer[used..used + 2].copy_from_slice(b", ");
                    used += 2;
                }
                buffer[used..used + name.len()].copy_from_slice(name.as_bytes());
                used += name.len();
            }
        }
        used
    }
}
const VARY_ALL:&str="Access-Control-Request-Headers, Access-Control-Request-Method, Authorization, Content-Type, Cookie, Origin, X-CSRF-Token";

enum ManagedValue {
    Ready(HeaderValue),
    CookieSlot(usize),
}
pub(super) struct ManagedEntry {
    name: ManagedHeader,
    value: ManagedValue,
}
pub(super) struct ManagedPlan {
    entries: Vec<ManagedEntry>,
    bytes: usize,
    byte_limit: usize,
    count_limit: usize,
    vary: VarySet,
    // Includes all planned values until they have moved into the charged map.
    _charge: Arc<Reservation>,
}
impl ManagedPlan {
    fn new(produced: &Produced) -> Result<Self, Diagnostic> {
        let layout = produced.wire.ok_or(Diagnostic::InvalidLimits)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(layout.managed_count)
            .map_err(|_| Diagnostic::Allocation)?;
        if entries.capacity() > layout.managed_count {
            return Err(Diagnostic::Allocation);
        }
        Ok(Self {
            entries,
            bytes: 0,
            byte_limit: layout.managed_bytes,
            count_limit: layout.managed_count,
            vary: VarySet::EMPTY,
            _charge: Arc::clone(&produced.headers.charge),
        })
    }
    fn preflight(&self, name: ManagedHeader, length: usize) -> Result<usize, Diagnostic> {
        let count = sum(self.entries.len(), 1)?;
        let bytes = sum(self.bytes, sum(sum(name.name().len(), length)?, 4)?)?;
        if count > self.count_limit || bytes > self.byte_limit {
            return Err(Diagnostic::HeaderLimit);
        }
        if name != ManagedHeader::SetCookie && self.entries.iter().any(|entry| entry.name == name) {
            return Err(Diagnostic::Fault);
        }
        Ok(bytes)
    }
    fn append(&mut self, name: ManagedHeader, value: &[u8]) -> Result<(), Diagnostic> {
        let bytes = self.preflight(name, value.len())?;
        let value = HeaderValue::from_bytes(value).map_err(|_| Diagnostic::HeaderLimit)?;
        self.entries.push(ManagedEntry {
            name,
            value: ManagedValue::Ready(value),
        });
        self.bytes = bytes;
        Ok(())
    }
    fn owned(&mut self, name: ManagedHeader, value: HeaderValue) -> Result<(), Diagnostic> {
        let bytes = self.preflight(name, value.as_bytes().len())?;
        self.entries.push(ManagedEntry {
            name,
            value: ManagedValue::Ready(value),
        });
        self.bytes = bytes;
        Ok(())
    }
    fn reserve_cookie(&mut self, max_bytes: usize) -> Result<usize, Diagnostic> {
        if max_bytes == 0 {
            return Err(Diagnostic::InvalidLimits);
        }
        let bytes = self.preflight(ManagedHeader::SetCookie, max_bytes)?;
        let index = self.entries.len();
        self.entries.push(ManagedEntry {
            name: ManagedHeader::SetCookie,
            value: ManagedValue::CookieSlot(max_bytes),
        });
        self.bytes = bytes;
        Ok(index)
    }
    fn fill_cookie(&mut self, index: usize, value: HeaderValue) -> Result<(), Diagnostic> {
        let entry = self.entries.get_mut(index).ok_or(Diagnostic::Fault)?;
        let ManagedValue::CookieSlot(maximum) = entry.value else {
            return Err(Diagnostic::Fault);
        };
        if value.as_bytes().len() > maximum {
            return Err(Diagnostic::HeaderLimit);
        }
        entry.value = ManagedValue::Ready(value);
        Ok(())
    }
    fn preflight_complete(&self) -> Result<(), Diagnostic> {
        if self
            .entries
            .iter()
            .any(|entry| matches!(entry.value, ManagedValue::CookieSlot(_)))
        {
            return Err(Diagnostic::Fault);
        }
        self.preflight_vary()
    }
    fn preflight_vary(&self) -> Result<(), Diagnostic> {
        let mut buffer = [0u8; VARY_ALL.len()];
        let length = self.vary.encode(&mut buffer);
        if length != 0 {
            self.preflight(ManagedHeader::Vary, length)?;
        }
        Ok(())
    }
    fn complete(mut self) -> Result<Self, Diagnostic> {
        self.preflight_complete()?;
        let mut buffer = [0u8; VARY_ALL.len()];
        let used = self.vary.encode(&mut buffer);
        if used != 0 {
            self.append(ManagedHeader::Vary, &buffer[..used])?;
        }
        Ok(self)
    }
    fn apply(mut self, wire: &mut ChargedWire) -> Result<(), Diagnostic> {
        // Drain only our private plan Vec. The native HeaderMap has no remove,
        // replace, drain, or clone path; each prepared value moves once.
        for entry in self.entries.drain(..) {
            let ManagedValue::Ready(value) = entry.value else {
                return Err(Diagnostic::Fault);
            };
            wire.headers_mut().append_owned(entry.name, value)?;
        }
        Ok(())
    }
}

pub(in crate::http_server) struct ResponseMeta {
    pub(in crate::http_server) status: StatusCode,
    pub(in crate::http_server) kind: BodyKind,
    pub(in crate::http_server) head: bool,
    pub(in crate::http_server) source: PolicySource,
    pub(in crate::http_server) origin: CompletionOrigin,
}
pub(in crate::http_server) struct PreparedResponse {
    produced: Produced,
    plan: ManagedPlan,
    status: StatusCode,
    head: bool,
    origin: CompletionOrigin,
    source: PolicySource,
    expected_body: Option<usize>,
}
impl PreparedResponse {
    pub(in crate::http_server) fn new(
        producer: &Producer,
        produced: Produced,
        meta: ResponseMeta,
    ) -> Result<Self, Diagnostic> {
        let ResponseMeta {
            status,
            kind,
            head,
            source,
            origin,
        } = meta;
        if !producer.owns(&produced) {
            return Err(Diagnostic::Fault);
        }
        if source != PolicySource::Public && status == StatusCode::NOT_MODIFIED {
            return Err(Diagnostic::Fault);
        }
        if kind == BodyKind::Html
            && produced
                .headers
                .entries
                .iter()
                .any(|h| h.name.eq_ignore_ascii_case(b"content-encoding"))
        {
            return Err(Diagnostic::Fault);
        }
        let mut plan = ManagedPlan::new(&produced)?;
        let no_representation = matches!(
            status,
            StatusCode::NO_CONTENT | StatusCode::RESET_CONTENT | StatusCode::NOT_MODIFIED
        );
        if !no_representation {
            if let Some(value) = kind.content_type() {
                plan.append(ManagedHeader::ContentType, value.as_bytes())?;
            }
        }
        plan.append(
            ManagedHeader::ContentSecurityPolicy,
            CONTENT_SECURITY_POLICY.as_bytes(),
        )?;
        plan.append(ManagedHeader::Nosniff, b"nosniff")?;
        if source != PolicySource::Public {
            plan.append(ManagedHeader::CacheControl, b"no-store")?;
        }
        plan.vary = VarySet::source(source);
        if status == StatusCode::RESET_CONTENT {
            plan.append(ManagedHeader::ContentLength, b"0")?;
        } else if head && !no_representation {
            let mut digits = [0; 20];
            let value = decimal(
                u64::try_from(produced.body().len()).map_err(|_| Diagnostic::InvalidLimits)?,
                &mut digits,
            );
            plan.append(ManagedHeader::ContentLength, value)?;
        }
        Ok(Self {
            produced,
            plan,
            status,
            head,
            origin,
            source,
            expected_body: None,
        })
    }
    pub(in crate::http_server) fn status(&self) -> StatusCode {
        self.status
    }
    pub(in crate::http_server) fn vary(&mut self, value: VarySet) {
        self.plan.vary = self.plan.vary.union(value);
    }
    pub(in crate::http_server) fn cors(
        &mut self,
        origin: Option<HeaderValue>,
        credentials: bool,
    ) -> Result<(), Diagnostic> {
        if let Some(origin) = origin {
            self.plan.owned(ManagedHeader::AllowOrigin, origin)?;
            if credentials {
                self.plan.append(ManagedHeader::AllowCredentials, b"true")?;
            }
        }
        self.vary(VarySet::ORIGIN);
        Ok(())
    }
    pub(in crate::http_server) fn allow(&mut self, value: &[u8]) -> Result<(), Diagnostic> {
        self.plan.append(ManagedHeader::Allow, value)
    }
    pub(in crate::http_server) fn preflight(
        &mut self,
        method: &[u8],
        headers: Option<&[u8]>,
        max_age: Option<&[u8]>,
    ) -> Result<(), Diagnostic> {
        if !self
            .plan
            .entries
            .iter()
            .any(|entry| entry.name == ManagedHeader::CacheControl)
        {
            self.plan.append(ManagedHeader::CacheControl, b"no-store")?;
        }
        self.plan.append(ManagedHeader::AllowMethods, method)?;
        if let Some(value) = headers {
            self.plan.append(ManagedHeader::AllowHeaders, value)?;
        }
        if let Some(value) = max_age {
            self.plan.append(ManagedHeader::MaxAge, value)?;
        }
        self.vary(VarySet::PREFLIGHT);
        Ok(())
    }
    pub(in crate::http_server) fn close(&mut self) -> Result<(), Diagnostic> {
        self.plan.append(ManagedHeader::Connection, b"close")
    }
    pub(in crate::http_server) fn cookie(&mut self, value: HeaderValue) -> Result<(), Diagnostic> {
        self.plan.owned(ManagedHeader::SetCookie, value)
    }
    pub(in crate::http_server) fn reserve_cookie(
        &mut self,
        max_bytes: usize,
    ) -> Result<usize, Diagnostic> {
        self.plan.reserve_cookie(max_bytes)
    }
    pub(in crate::http_server) fn take_session_cookie(
        mut self,
        owner: &crate::auth::LeaseOwner,
        seal: crate::auth::session::ResponseSeal,
        slot: usize,
    ) -> Result<Self, Diagnostic> {
        // Same-producer/invariant/managed-profile checks have already succeeded.
        // Replacement never obtains destructive delivery, even if it is 2xx.
        if self.origin != CompletionOrigin::HandlerOk {
            crate::auth::session::retire_http_delivery(owner);
            return Err(Diagnostic::Fault);
        }
        if !matches!(
            self.plan.entries.get(slot).map(|e| &e.value),
            Some(ManagedValue::CookieSlot(_))
        ) {
            return Err(Diagnostic::Fault);
        }
        if self.status == StatusCode::NOT_MODIFIED
            || self
                .produced
                .headers
                .entries
                .iter()
                .any(|h| h.name.eq_ignore_ascii_case(b"cache-control") && h.value != b"no-store")
        {
            crate::auth::session::retire_http_delivery(owner);
            return Err(Diagnostic::Fault);
        }
        if self.source == PolicySource::Public {
            self.plan.append(ManagedHeader::CacheControl, b"no-store")?;
            self.vary(VarySet::AUTH);
            self.source = PolicySource::Session;
        }
        self.vary(VarySet::COOKIE);
        self.plan.preflight_vary()?;
        let cookie =
            crate::auth::session::take_http_cookie(owner, seal).map_err(|_| Diagnostic::Fault)?;
        self.plan.fill_cookie(slot, cookie)?;
        self.vary(VarySet::COOKIE);
        Ok(self)
    }
    pub(in crate::http_server) fn finish(mut self) -> Result<ChargedWire, Diagnostic> {
        if self
            .expected_body
            .is_some_and(|n| self.produced.body().len() != n)
        {
            return Err(Diagnostic::Fault);
        }
        let no_representation = matches!(
            self.status,
            StatusCode::NO_CONTENT | StatusCode::RESET_CONTENT | StatusCode::NOT_MODIFIED
        );
        if no_representation {
            self.produced
                .headers
                .entries
                .retain(|h| !h.name.eq_ignore_ascii_case(b"content-encoding"));
        }
        if self.head || no_representation {
            self.produced.body.data = BodyData::Ordinary(Vec::new());
        }
        // The final typed plan is complete before the one native conversion.
        let plan = self.plan.complete()?;
        let mut wire = self.produced.into_wire()?;
        plan.apply(&mut wire)?;
        Ok(wire)
    }
    pub(super) fn terminal(
        producer: &Producer,
        produced: Produced,
        intent: TerminalIntent,
    ) -> Result<Self, Diagnostic> {
        let mut response = Self::new(
            producer,
            produced,
            ResponseMeta {
                status: StatusCode::from_u16(intent.reason.code())
                    .map_err(|_| Diagnostic::Fault)?,
                kind: BodyKind::Text,
                head: intent.head,
                source: intent.source,
                origin: CompletionOrigin::Replacement,
            },
        )?;
        if intent.source == PolicySource::Public {
            response
                .plan
                .append(ManagedHeader::CacheControl, b"no-store")?;
        }
        response.vary(intent.dependencies);
        if intent.close {
            response.close()?;
        }
        if let Some(origin) = intent.cors_origin {
            response.cors(Some(origin), intent.cors_credentials)?;
        }
        if let Some(allow) = intent.allow {
            response.plan.owned(ManagedHeader::Allow, allow)?;
        }
        if let Some(cookie) = intent.deletion {
            response.cookie(cookie)?;
            response.vary(VarySet::COOKIE);
        }
        if matches!(intent.reason, FallbackStatus::Unauthorized) {
            response.plan.append(
                ManagedHeader::WwwAuthenticate,
                if intent.source == PolicySource::Session {
                    b"Session"
                } else {
                    b"Bearer"
                },
            )?;
        }
        if let FallbackStatus::Limited { retry_seconds } = intent.reason {
            if retry_seconds == 0 {
                return Err(Diagnostic::InvalidLimits);
            }
            let mut digits = [0; 20];
            response.plan.append(
                ManagedHeader::RetryAfter,
                decimal(retry_seconds, &mut digits),
            )?;
        }
        Ok(response)
    }
}
fn decimal(mut value: u64, buffer: &mut [u8; 20]) -> &[u8] {
    let mut start = buffer.len();
    loop {
        start -= 1;
        buffer[start] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            return &buffer[start..];
        }
    }
}

// Closed framework status/reason/source and consumed-deletion obligation.
// It owns no issuer/serializer/mapper callback and cannot reissue secret material.
pub(in crate::http_server) struct TerminalIntent {
    reason: FallbackStatus,
    source: PolicySource,
    head: bool,
    close: bool,
    dependencies: VarySet,
    cors_origin: Option<HeaderValue>,
    cors_credentials: bool,
    allow: Option<HeaderValue>,
    deletion: Option<HeaderValue>,
    metadata_bytes: usize,
    metadata_count: usize,
    held_bytes: usize,
    held_count: usize,
}
impl TerminalIntent {
    pub(in crate::http_server) fn plain(reason: FallbackStatus) -> Self {
        Self {
            reason,
            source: PolicySource::Public,
            head: false,
            close: false,
            dependencies: VarySet::EMPTY,
            cors_origin: None,
            cors_credentials: false,
            allow: None,
            deletion: None,
            metadata_bytes: 0,
            metadata_count: 0,
            held_bytes: 0,
            held_count: 0,
        }
    }
    pub(super) fn profiled(reason: FallbackStatus, layout: super::wire::WireLayout) -> Self {
        let mut value = Self::plain(reason);
        value.metadata_bytes = layout.managed_bytes;
        value.metadata_count = layout.managed_count;
        value
    }
    fn accept_metadata(&mut self, name: ManagedHeader, length: usize) -> Result<(), Diagnostic> {
        let bytes = sum(self.held_bytes, sum(sum(name.name().len(), length)?, 4)?)?;
        let count = sum(self.held_count, 1)?;
        if bytes > self.metadata_bytes || count > self.metadata_count {
            return Err(Diagnostic::HeaderLimit);
        }
        self.held_bytes = bytes;
        self.held_count = count;
        Ok(())
    }
    pub(in crate::http_server) fn context(
        mut self,
        source: PolicySource,
        head: bool,
        dependencies: VarySet,
    ) -> Self {
        self.source = source;
        self.head = head;
        self.dependencies = dependencies;
        self
    }
    pub(in crate::http_server) fn cors(
        mut self,
        origin: HeaderValue,
        credentials: bool,
    ) -> Result<Self, Diagnostic> {
        if self.cors_origin.is_some() {
            return Err(Diagnostic::Fault);
        }
        self.accept_metadata(ManagedHeader::AllowOrigin, origin.as_bytes().len())?;
        self.cors_origin = Some(origin);
        self.cors_credentials = credentials;
        self.dependencies = self.dependencies.union(VarySet::ORIGIN);
        Ok(self)
    }
    pub(in crate::http_server) fn connection_close(mut self, close: bool) -> Self {
        self.close = close;
        self
    }
    pub(in crate::http_server) fn consumed_deletion(
        mut self,
        cookie: HeaderValue,
    ) -> Result<Self, Diagnostic> {
        if self.deletion.is_some() {
            return Err(Diagnostic::Fault);
        }
        self.accept_metadata(ManagedHeader::SetCookie, cookie.as_bytes().len())?;
        self.deletion = Some(cookie);
        Ok(self)
    }
    pub(in crate::http_server) fn allow(mut self, value: HeaderValue) -> Result<Self, Diagnostic> {
        if self.allow.is_some() {
            return Err(Diagnostic::Fault);
        }
        self.accept_metadata(ManagedHeader::Allow, value.as_bytes().len())?;
        self.allow = Some(value);
        Ok(self)
    }
    // The caller supplies an existing deadline/cancellation refusal, preserving
    // its original status instead of replacing it with a later Capacity error.
    pub(in crate::http_server) fn cancellation(mut self, reason: FallbackStatus) -> Self {
        self.reason = reason;
        self
    }
    pub(in crate::http_server) fn status(&self) -> FallbackStatus {
        self.reason
    }
    pub(super) fn valid(&self) -> bool {
        !matches!(self.reason, FallbackStatus::Limited { retry_seconds: 0 })
    }
}
pub(super) fn fallback_minimum_fits(count: usize, bytes: usize) -> bool {
    let fields = [
        (
            ManagedHeader::ContentType,
            b"text/plain; charset=utf-8".len(),
        ),
        (
            ManagedHeader::ContentSecurityPolicy,
            CONTENT_SECURITY_POLICY.len(),
        ),
        (ManagedHeader::Nosniff, b"nosniff".len()),
        (ManagedHeader::CacheControl, b"no-store".len()),
        (ManagedHeader::Connection, b"close".len()),
        (ManagedHeader::RetryAfter, 20),
        (ManagedHeader::WwwAuthenticate, b"Session".len()),
        (ManagedHeader::Vary, VARY_ALL.len()),
        (ManagedHeader::ContentLength, 20),
    ];
    let needed = fields.iter().try_fold(0, |n, (name, length)| {
        sum(n, sum(sum(name.name().len(), *length)?, 4)?)
    });
    count >= fields.len() && needed.is_ok_and(|n| bytes >= n)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::http_server) enum FrameworkBody {
    Csrf,
    Prelogin,
}
impl FrameworkBody {
    fn len(self) -> usize {
        crate::auth::csrf::FIXED_TOKEN_BODY_BYTES
    }
}
pub(in crate::http_server) struct FrameworkTicket {
    prepared: PreparedResponse,
    kind: FrameworkBody,
    cookie: Option<usize>,
}
pub(in crate::http_server) enum FrameworkError {
    Budget(Diagnostic),
    Csrf(crate::auth::csrf::Failure),
    Prelogin(crate::auth::prelogin::Failure),
}
impl FrameworkTicket {
    pub(in crate::http_server) fn reserve(
        producer: &Producer,
        kind: FrameworkBody,
        cookie_max: Option<usize>,
    ) -> Result<Self, Diagnostic> {
        if producer.limits.body_bytes < kind.len() {
            return Err(Diagnostic::BodyLimit);
        }
        if (kind == FrameworkBody::Prelogin) != cookie_max.is_some() {
            return Err(Diagnostic::InvalidLimits);
        }
        let produced = producer.empty()?;
        let mut prepared = PreparedResponse::new(
            producer,
            produced,
            ResponseMeta {
                status: StatusCode::OK,
                kind: BodyKind::Json,
                head: false,
                source: if kind == FrameworkBody::Csrf {
                    PolicySource::Session
                } else {
                    PolicySource::Prelogin
                },
                origin: CompletionOrigin::HandlerOk,
            },
        )?;
        prepared.expected_body = Some(kind.len());
        prepared.vary(VarySet::LOGIN);
        if kind == FrameworkBody::Csrf {
            prepared.plan.append(ManagedHeader::ContentLength, b"60")?;
        }
        let cookie = cookie_max
            .map(|maximum| prepared.reserve_cookie(maximum))
            .transpose()?;
        prepared.plan.preflight_vary()?;
        Ok(Self {
            prepared,
            kind,
            cookie,
        })
    }
    pub(in crate::http_server) fn cors(
        mut self,
        origin: Option<HeaderValue>,
        credentials: bool,
    ) -> Result<Self, Diagnostic> {
        self.prepared.cors(origin, credentials)?;
        self.prepared.plan.preflight_vary()?;
        Ok(self)
    }
    pub(in crate::http_server) fn dependencies(
        mut self,
        value: VarySet,
    ) -> Result<Self, Diagnostic> {
        self.prepared.vary(value);
        self.prepared.plan.preflight_vary()?;
        Ok(self)
    }
    fn adopt(mut self, body: Bytes) -> Result<PreparedResponse, Diagnostic> {
        if body.len() != self.kind.len() {
            return Err(Diagnostic::Fault);
        }
        self.prepared.produced.body.data = BodyData::Framework(body);
        Ok(self.prepared)
    }
    pub(in crate::http_server) fn take_csrf(
        self,
        store: &crate::auth::csrf::Inner,
        owner: &crate::auth::csrf::TokenOwner,
        seal: crate::auth::csrf::TokenSeal,
        binding: &crate::auth::session::CsrfBinding,
    ) -> Result<PreparedResponse, FrameworkError> {
        if self.kind != FrameworkBody::Csrf {
            return Err(FrameworkError::Budget(Diagnostic::Fault));
        }
        self.prepared
            .plan
            .preflight_vary()
            .map_err(FrameworkError::Budget)?;
        let body = store
            .take_wire(owner, seal, binding)
            .map_err(FrameworkError::Csrf)?;
        self.adopt(body).map_err(FrameworkError::Budget)
    }
    pub(in crate::http_server) fn take_prelogin(
        self,
        pool: &crate::auth::prelogin::Pool,
        owner: &crate::auth::prelogin::HttpOwner,
        seal: crate::auth::prelogin::HttpSeal,
    ) -> Result<PreparedResponse, FrameworkError> {
        if self.kind != FrameworkBody::Prelogin {
            return Err(FrameworkError::Budget(Diagnostic::Fault));
        }
        let slot = self
            .cookie
            .ok_or(FrameworkError::Budget(Diagnostic::Fault))?;
        self.prepared
            .plan
            .preflight_vary()
            .map_err(FrameworkError::Budget)?;
        let material = pool
            .take_wire(owner, seal)
            .map_err(FrameworkError::Prelogin)?;
        let mut prepared = self.adopt(material.body).map_err(FrameworkError::Budget)?;
        prepared
            .plan
            .fill_cookie(slot, material.cookie)
            .map_err(FrameworkError::Budget)?;
        Ok(prepared)
    }
}
impl Produced {
    pub(in crate::http_server) fn append_user_header(
        &mut self,
        name: &str,
        value: &[u8],
        html: bool,
    ) -> Result<(), Diagnostic> {
        self.headers.preflight(name.as_bytes(), value)?;
        // Validate the legacy grammar/reserved policy without constructing a
        // native HeaderName/HeaderValue or copying any rejected payload.
        if name.is_empty()
            || name.len() > 65535
            || !name.bytes().all(token)
            || value.iter().any(|b| (*b < 32 && *b != b'\t') || *b == 127)
        {
            return Err(Diagnostic::HeaderLimit);
        }
        let reserved = [
            "content-length",
            "transfer-encoding",
            "content-type",
            "set-cookie",
            "cache-control",
            "vary",
            "www-authenticate",
            "content-security-policy",
            "content-security-policy-report-only",
            "x-content-type-options",
            "x-frame-options",
            "strict-transport-security",
            "clear-site-data",
        ];
        if reserved.iter().any(|r| name.eq_ignore_ascii_case(r))
            || name
                .get(..15)
                .is_some_and(|n| n.eq_ignore_ascii_case("access-control-"))
            || (html && name.eq_ignore_ascii_case("content-encoding"))
        {
            return Err(Diagnostic::HeaderLimit);
        }
        self.headers.append(name.as_bytes(), value)
    }
}
fn token(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)
}

#[cfg(test)]
#[path = "response_finalize_tests.rs"]
mod tests;

pub(super) fn ordinary_minimum_fits(limits: &Limits) -> Result<bool, Diagnostic> {
    let content_type = [
        BodyKind::Text,
        BodyKind::Binary,
        BodyKind::Json,
        BodyKind::Html,
    ]
    .iter()
    .filter_map(|kind| kind.content_type())
    .map(str::len)
    .max()
    .ok_or(Diagnostic::Fault)?;
    let mut digits = [0; 20];
    let length = decimal(
        u64::try_from(limits.body_bytes).map_err(|_| Diagnostic::InvalidLimits)?,
        &mut digits,
    )
    .len();
    let fields = [
        (ManagedHeader::ContentType, content_type),
        (
            ManagedHeader::ContentSecurityPolicy,
            CONTENT_SECURITY_POLICY.len(),
        ),
        (ManagedHeader::Nosniff, b"nosniff".len()),
        (ManagedHeader::ContentLength, length),
        (ManagedHeader::CacheControl, b"no-store".len()),
        (ManagedHeader::Vary, VARY_ALL.len()),
    ];
    let bytes = fields.iter().try_fold(0, |n, (name, length)| {
        sum(n, sum(sum(name.name().len(), *length)?, 4)?)
    })?;
    Ok(limits.managed_header_count >= fields.len() && limits.managed_header_bytes >= bytes)
}
