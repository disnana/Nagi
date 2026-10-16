//! One-way native conversion. No raw HeaderMap extraction or Clone operation.
//! Logical storage credits include pinned http1.5.0 conversion/growth peaks;
//! allocator size classes/RSS and universal native OOM recovery are excluded.
use super::*;
use axum::http::{HeaderMap, HeaderName, HeaderValue};

#[derive(Clone, Copy)]
pub(super) struct WireLayout {
    pub(super) peak_bytes: usize,
    count: usize,
    max_capacity: usize,
    managed_bytes: usize,
    managed_count: usize,
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
            size_of::<ChargedWire>(),
            size_of::<ChargedNativeHeaders>(),
            size_of::<WireLayout>(),
            size_of::<HeaderMap>(),
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
        })
    }
}

// These are framework-owned names, never an API for user reserved headers.
#[derive(Clone, Copy)]
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
}
impl ManagedHeader {
    fn name(self) -> &'static str {
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
}
impl ChargedNativeHeaders {
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
    pub(in crate::http_server) fn value(&self, name: &str) -> Option<&[u8]> {
        self.map.get(name).map(HeaderValue::as_bytes)
    }
}
pub(in crate::http_server) struct ChargedWire {
    headers: ChargedNativeHeaders,
    body: Bytes,
}
impl ChargedWire {
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
}
impl FallbackStatus {
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
        }
    }
    fn body(self) -> &'static [u8] {
        match self {
            Self::Limited { .. } => b"too many requests",
            Self::Capacity => b"service unavailable",
            Self::Internal => b"internal error",
            Self::Unauthorized => b"unauthorized",
            Self::Forbidden => b"forbidden",
            Self::BodyTimeout => b"request timeout",
            Self::BodyTooLarge => b"content too large",
            Self::Deadline => b"gateway timeout",
        }
    }
}
// Inline optional payload keeps the original charged owners without introducing
// a heap box or an uncharged allocation merely to shrink an enum discriminant.
pub(in crate::http_server) struct FallbackOutcome {
    response: Option<(u16, ChargedWire)>,
}
impl FallbackOutcome {
    fn close() -> Self {
        Self { response: None }
    }
    pub(in crate::http_server) fn is_close(&self) -> bool {
        self.response.is_none()
    }
    pub(in crate::http_server) fn into_response(self) -> Option<(u16, ChargedWire)> {
        self.response
    }
}
impl FallbackLedger {
    pub(in crate::http_server) fn checked(
        limits: FallbackLimits,
        accepted_connections: usize,
    ) -> Result<Self, Diagnostic> {
        if accepted_connections == 0
            || limits.responses < accepted_connections
            || limits.managed_header_count < 3
            || limits.managed_header_bytes < 101
        {
            return Err(Diagnostic::InvalidLimits);
        }
        // Closed bodies max19B, no user headers, no serializer or user mapper.
        // This creates a distinct Quota catalog, not another ordinary claim.
        let producer = Producer::checked_with(
            Limits {
                body_bytes: 19,
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
        )?;
        if product(producer.limits.charge, accepted_connections)? > limits.retained_bytes {
            return Err(Diagnostic::InvalidLimits);
        }
        Ok(Self { producer })
    }
    pub(in crate::http_server) fn produce(&self, status: FallbackStatus) -> FallbackOutcome {
        let retry = match status {
            FallbackStatus::Limited { retry_seconds: 0 } => return FallbackOutcome::close(),
            FallbackStatus::Limited { retry_seconds } => Some(retry_seconds),
            _ => None,
        };
        let result = self
            .producer
            .copy_input(status.body())
            .and_then(Produced::into_wire)
            .and_then(|mut wire| {
                wire.headers_mut()
                    .append_managed(ManagedHeader::CacheControl, b"no-store")?;
                wire.headers_mut()
                    .append_managed(ManagedHeader::ContentType, b"text/plain; charset=utf-8")?;
                if let Some(mut value) = retry {
                    let mut digits = [0u8; 20];
                    let mut start = digits.len();
                    while value != 0 {
                        start -= 1;
                        digits[start] = b'0' + (value % 10) as u8;
                        value /= 10;
                    }
                    wire.headers_mut()
                        .append_managed(ManagedHeader::RetryAfter, &digits[start..])?;
                }
                Ok(wire)
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
