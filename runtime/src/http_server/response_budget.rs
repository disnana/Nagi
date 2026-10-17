//! Private response producer; no public configuration/default or Context API.
//! Charged logical payload/capacity is bounded, not process RSS or user Serialize
//! temporary allocations. No rate, auth proof, Session seal or lease is minted.
use axum::body::Bytes;
use serde::Serialize;
use std::{io, mem::size_of, sync::Arc};

use crate::security_sf07_private::quota;
#[path = "response_finalize.rs"]
pub(super) mod finalize;
#[path = "response_wire.rs"]
pub(super) mod wire;
use quota::{
    try_reserve, Claim, Quota, QuotaError, Reservation, ResourceId, ResourceLimit, StartupError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Diagnostic {
    InvalidLimits,
    BodyLimit,
    HeaderLimit,
    WorkLimit,
    Allocation,
    Serialization,
    Capacity,
    Fault,
}
impl Diagnostic {
    // No serializer/error-message formatting or body/header data is retained.
    pub(super) fn message(self) -> &'static str {
        match self {
            Self::InvalidLimits => "invalid response budget",
            Self::BodyLimit => "response body limit",
            Self::HeaderLimit => "response header limit",
            Self::WorkLimit => "response serialization limit",
            Self::Allocation => "response allocation unavailable",
            Self::Serialization => "response serialization failed",
            Self::Capacity => "response capacity unavailable",
            Self::Fault => "response budget fault",
        }
    }
}

// Explicit private test profile only. All inputs checked; zero is not unlimited.
pub(super) struct Limits {
    pub body_bytes: usize,
    pub user_header_bytes: usize,
    pub user_header_count: usize,
    pub managed_header_bytes: usize,
    pub managed_header_count: usize,
    pub serialization_writes: usize,
    pub live_responses: usize,
    pub retained_bytes: usize,
}
struct CheckedLimits {
    body_bytes: usize,
    header_bytes: usize,
    header_count: usize,
    writes: usize,
    charge: usize,
    wire: Option<wire::WireLayout>,
    metadata: usize,
}
pub(super) struct Producer {
    limits: CheckedLimits,
    quota: Arc<Quota>,
    slots: ResourceId,
    bytes: ResourceId,
    #[cfg(test)]
    allocation_attempts: std::sync::atomic::AtomicUsize,
}

fn startup_diagnostic(error: StartupError) -> Diagnostic {
    match error {
        StartupError::Invalid => Diagnostic::InvalidLimits,
        StartupError::Allocation => Diagnostic::Allocation,
    }
}
fn product(a: usize, b: usize) -> Result<usize, Diagnostic> {
    a.checked_mul(b)
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or(Diagnostic::InvalidLimits)
}
fn sum(a: usize, b: usize) -> Result<usize, Diagnostic> {
    a.checked_add(b)
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or(Diagnostic::InvalidLimits)
}
impl Producer {
    #[cfg(test)]
    pub(super) fn checked(limits: Limits) -> Result<Self, Diagnostic> {
        Self::checked_with(limits, false, false, 0, 1)
    }
    #[cfg(test)]
    pub(super) fn checked_wire(limits: Limits) -> Result<Self, Diagnostic> {
        Self::checked_with(limits, true, false, 0, 1)
    }
    // Explicit production-shaped bridge planning. The ordinary count and bytes
    // are independent ceilings: no L * Q startup requirement is introduced.
    pub(super) fn checked_attached(
        limits: Limits,
        connections: usize,
        external_catalog_bytes: usize,
    ) -> Result<Self, Diagnostic> {
        if !finalize::ordinary_minimum_fits(&limits)? {
            return Err(Diagnostic::InvalidLimits);
        }
        let metadata = wire::bridge_metadata(
            connections,
            external_catalog_bytes,
            limits.managed_header_bytes,
        )?;
        Self::checked_with(limits, true, false, metadata, 1)
    }
    fn checked_with(
        limits: Limits,
        native: bool,
        empty_user: bool,
        bridge: usize,
        guaranteed_responses: usize,
    ) -> Result<Self, Diagnostic> {
        let values = [
            guaranteed_responses,
            limits.body_bytes,
            limits.managed_header_bytes,
            limits.managed_header_count,
            limits.serialization_writes,
            limits.live_responses,
            limits.retained_bytes,
        ];
        if values.iter().any(|n| *n == 0 || *n > isize::MAX as usize) {
            return Err(Diagnostic::InvalidLimits);
        }
        if (!empty_user && (limits.user_header_bytes == 0 || limits.user_header_count == 0))
            || (empty_user && (limits.user_header_bytes != 0 || limits.user_header_count != 0))
        {
            return Err(Diagnostic::InvalidLimits);
        }
        let wire = if native {
            Some(wire::WireLayout::checked(&limits)?)
        } else {
            None
        };
        let all_headers = sum(limits.user_header_count, limits.managed_header_count)?;
        let header_metadata = all_headers
            .checked_mul(size_of::<OwnedHeader>())
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or(Diagnostic::InvalidLimits)?;
        let payload = sum(
            limits.body_bytes,
            sum(limits.user_header_bytes, limits.managed_header_bytes)?,
        )?;
        // Fixed ownership metadata is part of the private logical claim. Actual
        // allocator size classes/overhead are outside this logical byte bound.
        let fixed = size_of::<ChargedBody>()
            + size_of::<ChargedHeaders>()
            + size_of::<Reservation>()
            + 2 * size_of::<usize>();
        let charge = sum(sum(payload, header_metadata)?, fixed)?;
        let charge = sum(charge, wire.map_or(0, |layout| layout.peak_bytes))?;
        // This producer owns its Quota allocation. External catalog credit
        // excludes this receipt, which is deducted once for ordinary/fallback.
        let fixed_metadata = sum(bridge, sum(size_of::<Self>(), 2 * size_of::<usize>())?)?;
        let planned_catalog = Quota::retained_catalog_minimum(2).map_err(startup_diagnostic)?;
        let planned_metadata = sum(fixed_metadata, planned_catalog)?;
        let guaranteed = product(charge, guaranteed_responses)?;
        if sum(planned_metadata, guaranteed)? > limits.retained_bytes {
            return Err(Diagnostic::InvalidLimits);
        }
        let available = limits
            .retained_bytes
            .checked_sub(planned_metadata)
            .ok_or(Diagnostic::InvalidLimits)?;
        let catalog_limit = limits
            .retained_bytes
            .checked_sub(sum(fixed_metadata, guaranteed)?)
            .ok_or(Diagnostic::InvalidLimits)?;
        let (quota, receipt) = Quota::checked_catalog_retained(
            &[
                ResourceLimit::new(limits.live_responses),
                ResourceLimit::new(available),
            ],
            2,
            catalog_limit,
        )
        .map_err(startup_diagnostic)?;
        let metadata = sum(fixed_metadata, receipt.bytes())?;
        // The same capacity bound backs both the minted byte resource and Sf.
        if receipt.bytes() != planned_catalog
            || sum(metadata, guaranteed)? > limits.retained_bytes
            || limits.retained_bytes.checked_sub(metadata) != Some(available)
        {
            return Err(Diagnostic::Fault);
        }
        let slots = quota.resource_id(0).ok_or(Diagnostic::Fault)?;
        let bytes = quota.resource_id(1).ok_or(Diagnostic::Fault)?;
        Ok(Self {
            limits: CheckedLimits {
                body_bytes: limits.body_bytes,
                header_bytes: limits.user_header_bytes,
                header_count: limits.user_header_count,
                writes: limits.serialization_writes,
                charge,
                wire,
                metadata,
            },
            quota,
            slots,
            bytes,
            #[cfg(test)]
            allocation_attempts: std::sync::atomic::AtomicUsize::new(0),
        })
    }
    fn reserve(&self) -> Result<Arc<Reservation>, Diagnostic> {
        let claim = try_reserve(
            &self.quota,
            &[
                Claim::new(self.slots, 1),
                Claim::new(self.bytes, self.limits.charge),
            ],
        )
        .map_err(|e| match e {
            QuotaError::Capacity => Diagnostic::Capacity,
            QuotaError::Fault | QuotaError::Invalid => Diagnostic::Fault,
        })?;
        Ok(Arc::new(claim))
    }
    fn allocate<T>(&self, target: &mut Vec<T>, additional: usize) -> Result<(), Diagnostic> {
        if additional > target.capacity().saturating_sub(target.len()) {
            #[cfg(test)]
            self.allocation_attempts
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            target
                .try_reserve_exact(additional)
                .map_err(|_| Diagnostic::Allocation)?;
        }
        Ok(())
    }
    fn finish(&self, data: Vec<u8>, charge: Arc<Reservation>) -> Result<Produced, Diagnostic> {
        if data.len() > self.limits.body_bytes {
            return Err(Diagnostic::BodyLimit);
        }
        if data.capacity() > self.limits.body_bytes {
            return Err(Diagnostic::Allocation);
        }
        let body = ChargedBody {
            data: BodyData::Ordinary(data),
            charge,
        };
        let mut headers = Vec::new();
        self.allocate(&mut headers, self.limits.header_count)?;
        if headers.capacity() > self.limits.header_count {
            return Err(Diagnostic::Allocation);
        }
        Ok(Produced {
            headers: ChargedHeaders {
                entries: headers,
                used_bytes: 0,
                used_capacity: 0,
                cap_bytes: self.limits.header_bytes,
                cap_count: self.limits.header_count,
                charge: Arc::clone(&body.charge),
            },
            body,
            wire: self.limits.wire,
            owner: Arc::clone(&self.quota),
        })
    }
    pub(super) fn metadata_bytes(&self) -> usize {
        self.limits.metadata
    }
    pub(super) fn response_charge(&self) -> usize {
        self.limits.charge
    }
    pub(super) fn empty(&self) -> Result<Produced, Diagnostic> {
        self.copy_input(&[])
    }
    pub(super) fn html(
        &self,
        document: &crate::html::HtmlDocument,
    ) -> Result<Produced, Diagnostic> {
        self.copy_input(document.encoded_for_response().as_bytes())
    }
    pub(super) fn owns(&self, value: &Produced) -> bool {
        Arc::ptr_eq(&self.quota, &value.owner)
    }
    pub(super) fn bytes(&self, input: &[u8]) -> Result<Produced, Diagnostic> {
        self.copy_input(input)
    }
    fn copy_input(&self, input: &[u8]) -> Result<Produced, Diagnostic> {
        if input.len() > self.limits.body_bytes {
            return Err(Diagnostic::BodyLimit);
        }
        let charge = self.reserve()?;
        let mut data = Vec::new();
        self.allocate(&mut data, input.len())?;
        if data.capacity() > self.limits.body_bytes {
            return Err(Diagnostic::Allocation);
        }
        data.extend_from_slice(input);
        self.finish(data, charge)
    }
    pub(super) fn text(&self, input: &str) -> Result<Produced, Diagnostic> {
        self.bytes(input.as_bytes())
    }
    pub(super) fn json<T: Serialize + ?Sized>(&self, input: &T) -> Result<Produced, Diagnostic> {
        // Claim precedes any writer growth. serde_json writes directly into the
        // bounded sink; no to_vec/string or post-allocation truncation occurs.
        let charge = self.reserve()?;
        let mut writer = FiniteWriter {
            producer: self,
            data: Vec::new(),
            writes: 0,
            failure: None,
        };
        let serialized = serde_json::to_writer(&mut writer, input);
        if let Some(failure) = writer.failure {
            return Err(failure);
        }
        if serialized.is_err() {
            return Err(Diagnostic::Serialization);
        }
        self.finish(writer.data, charge)
    }
}

struct FiniteWriter<'a> {
    producer: &'a Producer,
    data: Vec<u8>,
    writes: usize,
    failure: Option<Diagnostic>,
}
impl FiniteWriter<'_> {
    fn fail(&mut self, error: Diagnostic) -> io::Error {
        self.failure.get_or_insert(error);
        io::ErrorKind::Other.into()
    }
}
impl io::Write for FiniteWriter<'_> {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if let Some(error) = self.failure {
            return Err(self.fail(error));
        }
        // Empty writes consume work too. A serializer cannot erase a failed
        // write or obtain unbounded sink calls through a zero-length input.
        let Some(writes) = self.writes.checked_add(1) else {
            return Err(self.fail(Diagnostic::WorkLimit));
        };
        if writes > self.producer.limits.writes {
            return Err(self.fail(Diagnostic::WorkLimit));
        }
        self.writes = writes;
        let Some(length) = self.data.len().checked_add(input.len()) else {
            return Err(self.fail(Diagnostic::BodyLimit));
        };
        if length > self.producer.limits.body_bytes {
            return Err(self.fail(Diagnostic::BodyLimit));
        }
        if let Err(error) = self.producer.allocate(&mut self.data, input.len()) {
            return Err(self.fail(error));
        }
        if self.data.capacity() > self.producer.limits.body_bytes {
            return Err(self.fail(Diagnostic::Allocation));
        }
        self.data.extend_from_slice(input);
        Ok(input.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        match self.failure {
            Some(error) => Err(self.fail(error)),
            None => Ok(()),
        }
    }
}

// Payload fields precede the final Arc claim, so payload disposal precedes quota
// release. No raw Vec extraction that could shed the charge is provided.
enum BodyData {
    Ordinary(Vec<u8>),
    Framework(Bytes),
}
impl AsRef<[u8]> for BodyData {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Ordinary(data) => data,
            Self::Framework(bytes) => bytes.as_ref(),
        }
    }
}
pub(super) struct ChargedBody {
    data: BodyData,
    charge: Arc<Reservation>,
}
impl AsRef<[u8]> for ChargedBody {
    fn as_ref(&self) -> &[u8] {
        self.data.as_ref()
    }
}
pub(super) struct OwnedHeader {
    name: Vec<u8>,
    value: Vec<u8>,
}
pub(super) struct ChargedHeaders {
    entries: Vec<OwnedHeader>,
    used_bytes: usize,
    used_capacity: usize,
    cap_bytes: usize,
    cap_count: usize,
    charge: Arc<Reservation>,
}
impl ChargedHeaders {
    pub(super) fn preflight(&self, name: &[u8], value: &[u8]) -> Result<usize, Diagnostic> {
        let count = self
            .entries
            .len()
            .checked_add(1)
            .ok_or(Diagnostic::HeaderLimit)?;
        let added = name
            .len()
            .checked_add(value.len())
            .and_then(|n| n.checked_add(4))
            .ok_or(Diagnostic::HeaderLimit)?;
        let bytes = self
            .used_bytes
            .checked_add(added)
            .ok_or(Diagnostic::HeaderLimit)?;
        if count > self.cap_count || bytes > self.cap_bytes {
            return Err(Diagnostic::HeaderLimit);
        }
        Ok(bytes)
    }
    pub(super) fn append(&mut self, name: &[u8], value: &[u8]) -> Result<(), Diagnostic> {
        let bytes = self.preflight(name, value)?;
        // Grammar/reserved-name checks remain the HTTP append_header/finalizer's
        // responsibility. This private storage never authorizes managed headers.
        let mut owned_name = Vec::new();
        owned_name
            .try_reserve_exact(name.len())
            .map_err(|_| Diagnostic::Allocation)?;
        let mut owned_value = Vec::new();
        owned_value
            .try_reserve_exact(value.len())
            .map_err(|_| Diagnostic::Allocation)?;
        let retained = sum(
            self.used_capacity,
            sum(sum(owned_name.capacity(), owned_value.capacity())?, 4)?,
        )?;
        if retained > self.cap_bytes {
            return Err(Diagnostic::HeaderLimit);
        }
        owned_name.extend_from_slice(name);
        owned_value.extend_from_slice(value);
        self.entries.push(OwnedHeader {
            name: owned_name,
            value: owned_value,
        });
        self.used_bytes = bytes;
        self.used_capacity = retained;
        Ok(())
    }
}
pub(super) struct Produced {
    pub headers: ChargedHeaders,
    body: ChargedBody,
    wire: Option<wire::WireLayout>,
    owner: Arc<Quota>,
}
impl Produced {
    pub(super) fn body(&self) -> &[u8] {
        self.body.as_ref()
    }
    pub(super) fn into_wire(self) -> Result<wire::ChargedWire, Diagnostic> {
        wire::convert(self)
    }
    // Legacy private oracle only; production transfer must use into_wire.
    #[cfg(test)]
    pub(super) fn into_parts(self) -> (ChargedHeaders, Bytes) {
        // Bytes owns ChargedBody. Cloned native frames retain the same claim
        // until their actual final owner drops; taking a frame is not disposal.
        (self.headers, Bytes::from_owner(self.body))
    }
}

#[cfg(test)]
#[path = "response_budget_tests.rs"]
mod tests;
