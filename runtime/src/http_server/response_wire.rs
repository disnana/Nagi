//! One-way native conversion. No raw HeaderMap extraction or Clone operation.
//! Logical storage credits include pinned http1.5.0 conversion/growth peaks;
//! allocator size classes/RSS and universal native OOM recovery are excluded.
use super::*;
use crate::auth::FailureKind;
use axum::http::{HeaderMap, HeaderName, HeaderValue};

#[derive(Clone, Copy)]
pub(super) struct WireLayout {
    pub(super) peak_bytes: usize,
    pub(super) count: usize,
    max_capacity: usize,
    pub(super) managed_bytes: usize,
    pub(super) managed_count: usize,
    user_bytes: usize,
    user_count: usize,
}
fn product(a: usize, b: usize) -> Result<usize, Diagnostic> {
    a.checked_mul(b)
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or(Diagnostic::InvalidLimits)
}
impl WireLayout {
    pub(super) fn checked(limits: &Limits) -> Result<Self, Diagnostic> {
        let count = sum(limits.user_header_count, limits.managed_header_count)?;
        // http1.5.0 to_raw_capacity/usable_capacity/MAX_SIZE. Admission also
        // covers collision-driven yellow growth, not only initial reserve.
        let initial = sum(count, count / 3)?
            .checked_next_power_of_two()
            .ok_or(Diagnostic::InvalidLimits)?;
        const MAX_RAW: usize = 1 << 15;
        if initial > MAX_RAW {
            return Err(Diagnostic::InvalidLimits);
        }
        let raw = sum(product(count, 5)?, 1)?
            .checked_next_power_of_two()
            .ok_or(Diagnostic::InvalidLimits)?
            .min(MAX_RAW);
        // Conservative per-element logical credits dominate the pinned Pos,
        // Bucket<HeaderValue>, Option<Links> and ExtraValue representations.
        // Two tables cover old+new indices/entry storage during native growth.
        let bucket = sum(
            sum(size_of::<HeaderName>(), size_of::<HeaderValue>())?,
            product(8, size_of::<usize>())?,
        )?;
        let tables = product(
            product(raw, 2)?,
            sum(bucket, product(2, size_of::<usize>())?)?,
        )?;
        // Extra-value Vec push growth: both old/new capacities plus minimum
        // startup slack. No private HeaderMap removal/drain/clone path exists.
        let extras = product(
            sum(product(count, 4)?, 4)?,
            sum(size_of::<HeaderValue>(), product(6, size_of::<usize>())?)?,
        )?;
        // Original ChargedHeaders remain alive through conversion. Native
        // name/value storage and one parsing scratch copy are additional.
        let payload = product(
            sum(limits.user_header_bytes, limits.managed_header_bytes)?,
            2,
        )?;
        let fixed = [
            size_of::<Produced>(),
            size_of::<super::super::response_public::Response>(),
            size_of::<super::super::response_public::HttpResponse>(),
            size_of::<ChargedWire>(),
            size_of::<ChargedNativeHeaders>(),
            size_of::<WireLayout>(),
            size_of::<super::finalize::PreparedResponse>(),
            size_of::<super::finalize::ManagedPlan>(),
            product(
                limits.managed_header_count,
                size_of::<super::finalize::ManagedEntry>(),
            )?,
            size_of::<HeaderMap>(),
            size_of::<super::super::response_send::SendLedger>(),
            size_of::<super::super::response_send::ReceivedHeaders>(),
            product(4, size_of::<usize>())?, // two Arc control blocks
            product(8, size_of::<usize>())?,
        ]
        .into_iter()
        .try_fold(0, sum)?;
        Ok(Self {
            peak_bytes: sum(sum(tables, extras)?, sum(payload, fixed)?)?,
            count,
            max_capacity: raw - raw / 4,
            managed_bytes: limits.managed_header_bytes,
            managed_count: limits.managed_header_count,
            user_bytes: limits.user_header_bytes,
            user_count: limits.user_header_count,
        })
    }
}

// These are framework-owned names, never an API for user reserved headers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::http_server) enum ManagedHeader {
    ContentType,
    ContentLength,
    CacheControl,
    Vary,
    WwwAuthenticate,
    SetCookie,
    ContentSecurityPolicy,
    Nosniff,
    AllowOrigin,
    AllowCredentials,
    AllowMethods,
    AllowHeaders,
    MaxAge,
    RetryAfter,
    Connection,
    Allow,
}
impl ManagedHeader {
    fn is_managed(name: &str) -> bool {
        [
            Self::ContentType,
            Self::ContentLength,
            Self::CacheControl,
            Self::Vary,
            Self::WwwAuthenticate,
            Self::SetCookie,
            Self::ContentSecurityPolicy,
            Self::Nosniff,
            Self::AllowOrigin,
            Self::AllowCredentials,
            Self::AllowMethods,
            Self::AllowHeaders,
            Self::MaxAge,
            Self::RetryAfter,
            Self::Connection,
            Self::Allow,
        ]
        .into_iter()
        .any(|key| key.name().eq_ignore_ascii_case(name))
    }
    pub(in crate::http_server) fn name(self) -> &'static str {
        match self {
            Self::ContentType => "content-type",
            Self::ContentLength => "content-length",
            Self::CacheControl => "cache-control",
            Self::Vary => "vary",
            Self::WwwAuthenticate => "www-authenticate",
            Self::SetCookie => "set-cookie",
            Self::ContentSecurityPolicy => "content-security-policy",
            Self::Nosniff => "x-content-type-options",
            Self::AllowOrigin => "access-control-allow-origin",
            Self::AllowCredentials => "access-control-allow-credentials",
            Self::AllowMethods => "access-control-allow-methods",
            Self::AllowHeaders => "access-control-allow-headers",
            Self::MaxAge => "access-control-max-age",
            Self::RetryAfter => "retry-after",
            Self::Connection => "connection",
            Self::Allow => "allow",
        }
    }
}

pub(in crate::http_server) struct ChargedNativeHeaders {
    map: HeaderMap,
    layout: WireLayout,
    managed_bytes: usize,
    managed_count: usize,
    owner: Arc<Quota>,
    // Drop native names/values/container before releasing the shared claim.
    charge: Arc<Reservation>,
    fault: Option<Diagnostic>,
}
impl ChargedNativeHeaders {
    #[cfg(test)]
    pub(in crate::http_server) fn append_managed(
        &mut self,
        name: ManagedHeader,
        value: &[u8],
    ) -> Result<(), Diagnostic> {
        let name = name.name();
        let count = sum(self.managed_count, 1)?;
        let bytes = sum(self.managed_bytes, sum(sum(name.len(), value.len())?, 4)?)?;
        if count > self.layout.managed_count || bytes > self.layout.managed_bytes {
            return Err(Diagnostic::HeaderLimit);
        }
        let value = HeaderValue::from_bytes(value).map_err(|_| Diagnostic::HeaderLimit)?;
        self.append_owned_checked(name, value, count, bytes)
    }
    pub(in crate::http_server) fn append_owned(
        &mut self,
        name: ManagedHeader,
        value: HeaderValue,
    ) -> Result<(), Diagnostic> {
        let name = name.name();
        let count = sum(self.managed_count, 1)?;
        let bytes = sum(
            self.managed_bytes,
            sum(sum(name.len(), value.as_bytes().len())?, 4)?,
        )?;
        if count > self.layout.managed_count || bytes > self.layout.managed_bytes {
            return Err(Diagnostic::HeaderLimit);
        }
        self.append_owned_checked(name, value, count, bytes)
    }
    fn append_owned_checked(
        &mut self,
        name: &'static str,
        value: HeaderValue,
        count: usize,
        bytes: usize,
    ) -> Result<(), Diagnostic> {
        self.map
            .try_append(HeaderName::from_static(name), value)
            .map_err(|_| Diagnostic::HeaderLimit)?;
        self.check_capacity()?;
        self.managed_count = count;
        self.managed_bytes = bytes;
        Ok(())
    }
    fn check_capacity(&self) -> Result<(), Diagnostic> {
        if self.map.len() > self.layout.count || self.map.capacity() > self.layout.max_capacity {
            Err(Diagnostic::Fault)
        } else {
            Ok(())
        }
    }
    pub(in crate::http_server) fn map(&self) -> &HeaderMap {
        &self.map
    }
    pub(in crate::http_server) fn fault(&self) -> Option<Diagnostic> {
        self.fault
    }
    fn preflight_native(
        &self,
        name: &str,
        value: &HeaderValue,
        replace: bool,
    ) -> Result<(), Diagnostic> {
        self.preflight_value(name, value.as_bytes().len(), replace)
    }
    // Length-only admission runs before construction of a managed value.
    pub(in crate::http_server) fn preflight_value(
        &self,
        name: &str,
        value_len: usize,
        replace: bool,
    ) -> Result<(), Diagnostic> {
        let mut counts = [0usize; 2];
        let mut bytes = [0usize; 2];
        for (key, old) in self.map.iter() {
            if replace && key.as_str().eq_ignore_ascii_case(name) {
                continue;
            }
            let group = usize::from(ManagedHeader::is_managed(key.as_str()));
            counts[group] = sum(counts[group], 1)?;
            bytes[group] = sum(
                bytes[group],
                sum(sum(key.as_str().len(), old.as_bytes().len())?, 4)?,
            )?;
        }
        let group = usize::from(ManagedHeader::is_managed(name));
        counts[group] = sum(counts[group], 1)?;
        bytes[group] = sum(bytes[group], sum(sum(name.len(), value_len)?, 4)?)?;
        if counts[0] > self.layout.user_count
            || bytes[0] > self.layout.user_bytes
            || counts[1] > self.layout.managed_count
            || bytes[1] > self.layout.managed_bytes
        {
            return Err(Diagnostic::HeaderLimit);
        }
        Ok(())
    }
    // Framework mutation stays inside the charged map. Refusal is sticky and
    // observed at the one native handoff; rejected values never enter the map.
    pub(in crate::http_server) fn insert<K: axum::http::header::IntoHeaderName + AsRef<str>>(
        &mut self,
        name: K,
        value: HeaderValue,
    ) -> Option<HeaderValue> {
        if self.fault.is_some() {
            return None;
        }
        if let Err(error) = self.preflight_native(name.as_ref(), &value, true) {
            self.fault = Some(error);
            return None;
        }
        match self.map.try_insert(name, value) {
            Ok(old) => {
                if let Err(e) = self.check_capacity() {
                    self.fault = Some(e);
                }
                old
            }
            Err(_) => {
                self.fault = Some(Diagnostic::HeaderLimit);
                None
            }
        }
    }
    pub(in crate::http_server) fn append<K: axum::http::header::IntoHeaderName + AsRef<str>>(
        &mut self,
        name: K,
        value: HeaderValue,
    ) -> bool {
        if self.fault.is_some() {
            return false;
        }
        if let Err(error) = self.preflight_native(name.as_ref(), &value, false) {
            self.fault = Some(error);
            return false;
        }
        match self.map.try_append(name, value) {
            Ok(added) => {
                if let Err(e) = self.check_capacity() {
                    self.fault = Some(e);
                }
                added
            }
            Err(_) => {
                self.fault = Some(Diagnostic::HeaderLimit);
                false
            }
        }
    }
    pub(in crate::http_server) fn remove<K: axum::http::header::AsHeaderName>(
        &mut self,
        name: K,
    ) -> Option<HeaderValue> {
        self.map.remove(name)
    }
    #[cfg(test)]
    pub(in crate::http_server) fn inject_fault(&mut self, error: Diagnostic) {
        self.fault = Some(error);
    }
    #[cfg(test)]
    pub(in crate::http_server) fn value(&self, name: &str) -> Option<&[u8]> {
        self.map.get(name).map(HeaderValue::as_bytes)
    }
}
pub(in crate::http_server) struct ChargedWire {
    headers: ChargedNativeHeaders,
    body: Bytes,
}
impl ChargedWire {
    pub(in crate::http_server) fn from_parts(headers: ChargedNativeHeaders, body: Bytes) -> Self {
        Self { headers, body }
    }
    pub(in crate::http_server) fn headers_mut(&mut self) -> &mut ChargedNativeHeaders {
        &mut self.headers
    }
    pub(in crate::http_server) fn is_owned_by(&self, producer: &Producer) -> bool {
        Arc::ptr_eq(&self.headers.owner, &producer.quota)
    }
    pub(in crate::http_server) fn into_parts(self) -> (ChargedNativeHeaders, Bytes) {
        // Deliberately returns the charged wrapper, never a native HeaderMap.
        (self.headers, self.body)
    }
    // The only native-map handoff: the connection-owned slot is charged BEFORE
    // the map escapes this wrapper. Driver/receive own its subsequent lifetime.
    pub(in crate::http_server) fn transfer_native(
        self,
        ledger: &super::super::response_send::SendLedger,
        status: axum::http::StatusCode,
    ) -> Result<
        axum::http::Response<super::super::BufferedBody>,
        super::super::response_send::SendFault,
    > {
        use super::super::response_send::SendFault;
        if self.headers.fault.is_some() {
            return Err(SendFault::Exhausted);
        }
        if !ledger.accepts(&self) {
            return Err(SendFault::Foreign);
        }
        ledger.publish(Arc::clone(&self.headers.charge))?;
        let (headers, body) = self.into_parts();
        let ChargedNativeHeaders { map, .. } = headers;
        let mut response = axum::http::Response::new(super::super::BufferedBody {
            bytes: body,
            sending: None,
        });
        *response.status_mut() = status;
        *response.headers_mut() = map;
        Ok(response)
    }
    pub(in crate::http_server) fn suppress_body_for_head(&mut self) {
        // Header container retains the same claim even with an empty body.
        self.body = Bytes::new();
    }
}
pub(super) fn convert(produced: Produced) -> Result<ChargedWire, Diagnostic> {
    let layout = produced.wire.ok_or(Diagnostic::InvalidLimits)?;
    let mut native = ChargedNativeHeaders {
        map: HeaderMap::new(),
        layout,
        managed_bytes: 0,
        managed_count: 0,
        owner: Arc::clone(&produced.owner),
        charge: Arc::clone(&produced.headers.charge),
        fault: None,
    };
    // The complete producer claim, including the conversion peak, was acquired
    // before the original body/header allocations. This API's try_reserve
    // detects size refusal; pinned internals still contain infallible Vec alloc.
    native
        .map
        .try_reserve(layout.count)
        .map_err(|_| Diagnostic::Allocation)?;
    for header in &produced.headers.entries {
        let name = HeaderName::from_bytes(&header.name).map_err(|_| Diagnostic::HeaderLimit)?;
        let value = HeaderValue::from_bytes(&header.value).map_err(|_| Diagnostic::HeaderLimit)?;
        native
            .map
            .try_append(name, value)
            .map_err(|_| Diagnostic::HeaderLimit)?;
        native.check_capacity()?;
    }
    // Only after conversion may original header/name buffers be disposed.
    // Native wrapper and Bytes::from_owner retain the same Reservation Arc.
    drop(produced.headers);
    Ok(ChargedWire {
        headers: native,
        body: Bytes::from_owner(produced.body),
    })
}

pub(in crate::http_server) struct FallbackLimits {
    pub responses: usize,
    pub retained_bytes: usize,
    pub managed_header_bytes: usize,
    pub managed_header_count: usize,
}
pub(in crate::http_server) struct FallbackLedger {
    producer: Producer,
}
#[derive(Clone, Copy)]
pub(in crate::http_server) enum FallbackStatus {
    Limited { retry_seconds: u64 },
    Capacity,
    Internal,
    Unauthorized,
    Forbidden,
    BodyTimeout,
    BodyTooLarge,
    Deadline,
    BadRequest,
    NotFound,
    MethodNotAllowed,
    NotImplemented,
    UriTooLong,
    UnsupportedMediaType,
    MisdirectedRequest,
    HeaderFieldsTooLarge,
    VersionNotSupported,
    Security(FailureKind),
}
impl FallbackStatus {
    pub(in crate::http_server) fn all() -> [Self; 23] {
        [
            Self::Limited {
                retry_seconds: u64::MAX,
            },
            Self::Capacity,
            Self::Internal,
            Self::Unauthorized,
            Self::Forbidden,
            Self::BodyTimeout,
            Self::BodyTooLarge,
            Self::Deadline,
            Self::BadRequest,
            Self::NotFound,
            Self::MethodNotAllowed,
            Self::NotImplemented,
            Self::UriTooLong,
            Self::UnsupportedMediaType,
            Self::MisdirectedRequest,
            Self::HeaderFieldsTooLarge,
            Self::VersionNotSupported,
            Self::Security(FailureKind::InvalidCredential),
            Self::Security(FailureKind::Denied),
            Self::Security(FailureKind::Expired),
            Self::Security(FailureKind::InvalidRequest),
            Self::Security(FailureKind::Unavailable),
            Self::Security(FailureKind::Internal),
        ]
    }
    pub(in crate::http_server) fn code(self) -> u16 {
        match self {
            Self::Limited { .. } => 429,
            Self::Capacity => 503,
            Self::Internal => 500,
            Self::Unauthorized => 401,
            Self::Forbidden => 403,
            Self::BodyTimeout => 408,
            Self::BodyTooLarge => 413,
            Self::Deadline => 504,
            Self::BadRequest => 400,
            Self::NotFound => 404,
            Self::MethodNotAllowed => 405,
            Self::NotImplemented => 501,
            Self::UriTooLong => 414,
            Self::UnsupportedMediaType => 415,
            Self::MisdirectedRequest => 421,
            Self::HeaderFieldsTooLarge => 431,
            Self::VersionNotSupported => 505,
            Self::Security(kind) => match kind {
                FailureKind::InvalidCredential | FailureKind::Expired => 401,
                FailureKind::Denied => 403,
                FailureKind::InvalidRequest => 400,
                FailureKind::Unavailable => 503,
                FailureKind::Internal => 500,
            },
        }
    }
    pub(in crate::http_server) fn body(self) -> &'static [u8] {
        match self {
            Self::Limited { .. } => b"too many requests",
            Self::Capacity => b"service unavailable",
            Self::Internal => b"internal error",
            Self::Unauthorized => b"unauthorized",
            Self::Forbidden => b"forbidden",
            Self::BodyTimeout => b"request timeout",
            Self::BodyTooLarge => b"content too large",
            Self::Deadline => b"gateway timeout",
            Self::BadRequest => b"Bad Request",
            Self::NotFound => b"Not Found",
            Self::MethodNotAllowed => b"Method Not Allowed",
            Self::NotImplemented => b"Not Implemented",
            Self::UriTooLong => b"URI Too Long",
            Self::UnsupportedMediaType => b"Unsupported Media Type",
            Self::MisdirectedRequest => b"Misdirected Request",
            Self::HeaderFieldsTooLarge => b"Request Header Fields Too Large",
            Self::VersionNotSupported => b"HTTP Version Not Supported",
            Self::Security(kind) => kind.message().as_bytes(),
        }
    }
}
// Inline optional payload keeps the original charged owners without introducing
// a heap box or an uncharged allocation merely to shrink an enum discriminant.
pub(in crate::http_server) struct FallbackOutcome {
    response: Option<(u16, ChargedWire)>,
}
impl FallbackOutcome {
    pub(in crate::http_server) fn close() -> Self {
        Self { response: None }
    }
    #[cfg(test)]
    pub(in crate::http_server) fn is_close(&self) -> bool {
        self.response.is_none()
    }
    pub(in crate::http_server) fn into_response(self) -> Option<(u16, ChargedWire)> {
        self.response
    }
}
impl FallbackLedger {
    #[cfg(test)]
    pub(in crate::http_server) fn allocation_attempts(&self) -> usize {
        self.producer
            .allocation_attempts
            .load(std::sync::atomic::Ordering::Relaxed)
    }
    #[cfg(test)]
    pub(in crate::http_server) fn accounting(&self) -> (usize, usize) {
        (
            self.producer.metadata_bytes(),
            self.producer.response_charge(),
        )
    }
    pub(in crate::http_server) fn owns_wire(&self, wire: &ChargedWire) -> bool {
        wire.is_owned_by(&self.producer)
    }
    pub(in crate::http_server) fn checked(
        limits: FallbackLimits,
        accepted_connections: usize,
    ) -> Result<Self, Diagnostic> {
        if accepted_connections == 0
            || limits.responses < accepted_connections
            || !super::finalize::fallback_minimum_fits(
                limits.managed_header_count,
                limits.managed_header_bytes,
            )
        {
            return Err(Diagnostic::InvalidLimits);
        }
        // Derive the closed maximum from actual terminal representations;
        // no prototype body length becomes a public support ceiling.
        let body_bytes = FallbackStatus::all()
            .iter()
            .map(|status| status.body().len())
            .max()
            .ok_or(Diagnostic::InvalidLimits)?;
        // No user headers, serializer, or mapper is invoked by this ledger.
        // This creates a distinct Quota catalog, not another ordinary claim.
        let producer = Producer::checked_with(
            Limits {
                body_bytes,
                user_header_bytes: 0,
                user_header_count: 0,
                managed_header_bytes: limits.managed_header_bytes,
                managed_header_count: limits.managed_header_count,
                serialization_writes: 1,
                live_responses: limits.responses,
                retained_bytes: limits.retained_bytes,
            },
            true,
            true,
            sum(
                size_of::<Self>(),
                product(
                    accepted_connections,
                    sum(
                        size_of::<super::finalize::TerminalIntent>(),
                        limits.managed_header_bytes,
                    )?,
                )?,
            )?,
            accepted_connections,
        )?;
        if sum(
            producer.limits.metadata,
            product(producer.limits.charge, accepted_connections)?,
        )? > limits.retained_bytes
        {
            return Err(Diagnostic::InvalidLimits);
        }
        Ok(Self { producer })
    }
    pub(in crate::http_server) fn intent(
        &self,
        status: FallbackStatus,
    ) -> super::finalize::TerminalIntent {
        super::finalize::TerminalIntent::profiled(
            status,
            self.producer
                .limits
                .wire
                .expect("checked fallback wire profile"),
        )
    }
    #[cfg(test)]
    pub(in crate::http_server) fn produce(&self, status: FallbackStatus) -> FallbackOutcome {
        self.terminal(super::finalize::TerminalIntent::plain(status))
    }
    pub(in crate::http_server) fn terminal(
        &self,
        intent: super::finalize::TerminalIntent,
    ) -> FallbackOutcome {
        // Reserve before any fallback allocation; consume one immutable intent
        // exactly once. No ordinary factory, serializer, or mapper retry exists.
        if !intent.valid() {
            return FallbackOutcome::close();
        }
        let status = intent.status();
        let result = self
            .producer
            .copy_input(status.body())
            .and_then(|produced| {
                super::finalize::PreparedResponse::terminal(&self.producer, produced, intent)?
                    .finish()
            });
        match result {
            Ok(wire) => FallbackOutcome {
                response: Some((status.code(), wire)),
            },
            Err(_) => FallbackOutcome::close(),
        }
    }
}

#[cfg(test)]
#[path = "response_wire_tests.rs"]
mod tests;

pub(super) fn bridge_metadata(
    connections: usize,
    external_catalog_bytes: usize,
    managed_bytes: usize,
) -> Result<usize, Diagnostic> {
    if connections == 0 {
        return Err(Diagnostic::InvalidLimits);
    }
    let fixed = [
        size_of::<super::super::response_send::SendLedger>(),
        size_of::<super::super::response_send::SendState>(),
        size_of::<super::super::response_send::ReceivedHeaders>(),
        size_of::<super::finalize::TerminalIntent>(),
        product(4, size_of::<usize>())?,
    ]
    .into_iter()
    .try_fold(0, sum)?;
    sum(
        external_catalog_bytes,
        product(connections, sum(fixed, managed_bytes)?)?,
    )
}
