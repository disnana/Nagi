//! Catalog-scoped retained startup accounting. This is not an RSS allocator.
use super::*;
use hyper::{body::Bytes, header::HeaderValue};
use rustls::{
    pki_types::{Der, ServerName, TrustAnchor},
    ClientConfig, RootCertStore,
};
use std::{mem::size_of, sync::Mutex};

/// Required trusted-host values. No defaults or additional numerical ceilings.
pub struct RetentionLimits {
    pub max_root_anchors: usize,
    pub max_root_bytes: usize,
    pub max_config_bytes: usize,
}
#[derive(Clone, Copy, Default, Debug, Eq, PartialEq)]
pub(in crate::http_client) struct Retained {
    pub(in crate::http_client) anchors: usize,
    pub(in crate::http_client) der: usize,
    pub(in crate::http_client) config: usize,
}
fn invalid() -> Failure {
    Failure::new(FailureKind::InvalidInput, false)
}
fn capacity() -> Failure {
    Failure::new(FailureKind::Capacity, false)
}
fn unavailable() -> Failure {
    Failure::new(FailureKind::Unavailable, false)
}
pub(in crate::http_client) fn sum(values: &[usize]) -> Result<usize, Failure> {
    values
        .iter()
        .try_fold(0usize, |n, value| n.checked_add(*value).ok_or_else(invalid))
}
pub(in crate::http_client) fn slots<T>(count: usize) -> Result<usize, Failure> {
    let bytes = count.checked_mul(size_of::<T>()).ok_or_else(invalid)?;
    if bytes > isize::MAX as usize {
        return Err(invalid());
    }
    Ok(bytes)
}
impl Retained {
    fn checked_add(self, rhs: Self) -> Result<Self, Failure> {
        Ok(Self {
            anchors: sum(&[self.anchors, rhs.anchors])?,
            der: sum(&[self.der, rhs.der])?,
            config: sum(&[self.config, rhs.config])?,
        })
    }
    fn checked_sub(self, rhs: Self) -> Option<Self> {
        Some(Self {
            anchors: self.anchors.checked_sub(rhs.anchors)?,
            der: self.der.checked_sub(rhs.der)?,
            config: self.config.checked_sub(rhs.config)?,
        })
    }
}
struct Book {
    used: Retained,
    closed: bool,
    fault: bool,
}
pub(in crate::http_client) struct RetentionBudget {
    limits: RetentionLimits,
    book: Mutex<Book>,
}
impl RetentionBudget {
    pub(super) fn new(limits: RetentionLimits) -> Result<Arc<Self>, Failure> {
        if limits.max_root_anchors == 0
            || limits.max_root_bytes == 0
            || limits.max_config_bytes == 0
        {
            return Err(invalid());
        }
        // The state is charged once for its entire Arc lifetime. It owns no claim
        // and has no reverse reference to any payload.
        let used = Retained {
            config: size_of::<Self>(),
            ..Retained::default()
        };
        if used.config > limits.max_config_bytes {
            return Err(capacity());
        }
        Ok(Arc::new(Self {
            limits,
            book: Mutex::new(Book {
                used,
                closed: false,
                fault: false,
            }),
        }))
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Book>, Failure> {
        match self.book.lock() {
            Ok(book) => Ok(book),
            Err(poison) => {
                let mut book = poison.into_inner();
                book.closed = true;
                book.fault = true;
                Err(Failure::new(FailureKind::Internal, false))
            }
        }
    }
    pub(in crate::http_client) fn ensure_open(&self) -> Result<(), Failure> {
        let book = self.lock()?;
        if book.fault {
            return Err(Failure::new(FailureKind::Internal, false));
        }
        if book.closed {
            return Err(Failure::new(FailureKind::Closing, false));
        }
        Ok(())
    }
    pub(super) fn close(&self) {
        let mut book = self.book.lock().unwrap_or_else(|p| p.into_inner());
        book.closed = true;
    }
    fn fits(&self, used: Retained) -> bool {
        used.anchors <= self.limits.max_root_anchors
            && used.der <= self.limits.max_root_bytes
            && used.config <= self.limits.max_config_bytes
    }
    pub(in crate::http_client) fn reserve(
        self: &Arc<Self>,
        amount: Retained,
    ) -> Result<RetentionClaim, Failure> {
        let mut book = self.lock()?;
        if book.fault {
            return Err(Failure::new(FailureKind::Internal, false));
        }
        if book.closed {
            return Err(Failure::new(FailureKind::Closing, false));
        }
        let used = book.used.checked_add(amount)?;
        if !self.fits(used) {
            return Err(capacity());
        }
        book.used = used;
        Ok(RetentionClaim {
            budget: Arc::clone(self),
            amount,
        })
    }
    #[cfg(test)]
    pub(in crate::http_client) fn used(&self) -> Retained {
        self.book.lock().unwrap().used
    }
}
// Put this guard after the data it protects, including in native owner wrappers.
pub(in crate::http_client) struct RetentionClaim {
    budget: Arc<RetentionBudget>,
    amount: Retained,
}
impl RetentionClaim {
    pub(in crate::http_client) fn budget(&self) -> &Arc<RetentionBudget> {
        &self.budget
    }
    pub(in crate::http_client) fn grow(&mut self, additional: Retained) -> Result<(), Failure> {
        let amount = self.amount.checked_add(additional)?;
        let mut book = self.budget.lock()?;
        if book.fault {
            return Err(Failure::new(FailureKind::Internal, false));
        }
        if book.closed {
            return Err(Failure::new(FailureKind::Closing, false));
        }
        let used = book.used.checked_add(additional)?;
        if !self.budget.fits(used) {
            return Err(capacity());
        }
        book.used = used;
        self.amount = amount;
        Ok(())
    }
    pub(in crate::http_client) fn split(&mut self, amount: Retained) -> Result<Self, Failure> {
        let Some(remaining) = self.amount.checked_sub(amount) else {
            let mut book = self.budget.book.lock().unwrap_or_else(|p| p.into_inner());
            book.closed = true;
            book.fault = true;
            return Err(Failure::new(FailureKind::Internal, false));
        };
        self.amount = remaining;
        Ok(Self {
            budget: Arc::clone(&self.budget),
            amount,
        })
    }
}
impl Drop for RetentionClaim {
    fn drop(&mut self) {
        let mut book = self.budget.book.lock().unwrap_or_else(|p| p.into_inner());
        match book.used.checked_sub(self.amount) {
            Some(used) => book.used = used,
            None => {
                book.closed = true;
                book.fault = true;
            }
        }
    }
}
struct RetainedHeaderBytes {
    bytes: Vec<u8>,
    // HeaderValue/Bytes clones retain this owner through their actual final Drop.
    _claim: RetentionClaim,
}
impl AsRef<[u8]> for RetainedHeaderBytes {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}
pub(in crate::http_client) fn header_charge(capacity: usize) -> Result<usize, Failure> {
    sum(&[slots::<u8>(capacity)?, size_of::<RetainedHeaderBytes>()])
}
pub(in crate::http_client) fn owned_header(
    bytes: Vec<u8>,
    claim: RetentionClaim,
) -> Result<HeaderValue, Failure> {
    HeaderValue::from_maybe_shared(Bytes::from_owner(RetainedHeaderBytes {
        bytes,
        _claim: claim,
    }))
    .map_err(|_| invalid())
}
#[cfg(test)]
thread_local! {
    static COPY_FAULT: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static DER_COPIES: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}
fn allocation_point() -> Result<(), Failure> {
    #[cfg(test)]
    COPY_FAULT.with(|fault| match fault.get() {
        Some(0) => {
            fault.set(None);
            Err(unavailable())
        }
        Some(n) => {
            fault.set(Some(n - 1));
            Ok(())
        }
        None => Ok(()),
    })?;
    Ok(())
}
fn copied(bytes: &[u8], claim: &mut RetentionClaim, root: bool) -> Result<Vec<u8>, Failure> {
    slots::<u8>(bytes.len())?;
    let mut out = Vec::new();
    allocation_point()?;
    out.try_reserve_exact(bytes.len())
        .map_err(|_| unavailable())?;
    let extra = out
        .capacity()
        .checked_sub(bytes.len())
        .ok_or_else(invalid)?;
    claim.grow(Retained {
        der: if root { extra } else { 0 },
        config: extra,
        ..Retained::default()
    })?;
    #[cfg(test)]
    if root {
        DER_COPIES.with(|counter| {
            let (n, cap) = counter.get();
            counter.set((n + 1, cap + out.capacity()));
        });
    }
    out.extend_from_slice(bytes);
    Ok(out)
}
fn preflight_roots(roots: &RootCertStore, claim: &mut RetentionClaim) -> Result<(), Failure> {
    let logical = roots.roots.iter().try_fold(0usize, |n, anchor| {
        sum(&[
            n,
            slots::<u8>(anchor.subject.as_ref().len())?,
            slots::<u8>(anchor.subject_public_key_info.as_ref().len())?,
            slots::<u8>(
                anchor
                    .name_constraints
                    .as_ref()
                    .map_or(0, |n| n.as_ref().len()),
            )?,
        ])
    })?;
    let vector_bytes = slots::<TrustAnchor<'static>>(roots.roots.len())?;
    claim.grow(Retained {
        der: logical,
        config: sum(&[logical, vector_bytes])?,
        ..Retained::default()
    })?;
    Ok(())
}
fn normalized(roots: &RootCertStore, claim: &mut RetentionClaim) -> Result<RootCertStore, Failure> {
    // The complete count/logical-byte/slot reservation precedes every copy.
    let vector_bytes = slots::<TrustAnchor<'static>>(roots.roots.len())?;
    let mut out = RootCertStore::empty();
    allocation_point()?;
    out.roots
        .try_reserve_exact(roots.roots.len())
        .map_err(|_| unavailable())?;
    claim.grow(Retained {
        config: slots::<TrustAnchor<'static>>(out.roots.capacity())?
            .checked_sub(vector_bytes)
            .ok_or_else(invalid)?,
        ..Retained::default()
    })?;
    for anchor in &roots.roots {
        let subject = Der::from(copied(anchor.subject.as_ref(), claim, true)?);
        let subject_public_key_info = Der::from(copied(
            anchor.subject_public_key_info.as_ref(),
            claim,
            true,
        )?);
        let name_constraints = anchor
            .name_constraints
            .as_ref()
            .map(|n| copied(n.as_ref(), claim, true).map(Der::from))
            .transpose()?;
        out.roots.push(TrustAnchor {
            subject,
            subject_public_key_info,
            name_constraints,
        });
    }
    Ok(out)
}

// A distinct immutable owner per accepted service, including equal numeric IDs
// or equal root contents. Neither a root union nor a content-addressed cache.
pub(in crate::http_client) struct FrozenTrust {
    config: Arc<ClientConfig>,
    host: url::Host<String>,
    server_name: ServerName<'static>,
    authority: HeaderValue,
    identity: ServiceIdentity,
    port: u16,
    _claim: RetentionClaim,
}
impl FrozenTrust {
    pub(super) fn checked<M>(
        budget: &Arc<RetentionBudget>,
        identity: ServiceIdentity,
        host: url::Host<String>,
        port: u16,
        roots: RootCertStore,
        extra_config: usize,
        validate: impl FnOnce(&url::Host<String>) -> Result<M, Failure>,
    ) -> Result<(Arc<Self>, M), Failure> {
        // Count is tested before traversing DER or copying any configuration.
        // Declare the reservation before all accepted partial data: on error
        // their actual Drop always precedes the reservation's refund.
        let mut reservation = budget.reserve(Retained {
            anchors: roots.roots.len(),
            config: extra_config,
            ..Retained::default()
        })?;
        if port == 0 {
            return Err(invalid());
        }
        let (name, authority_name, host_capacity, expect_ip) = match &host {
            url::Host::Domain(name) => {
                if name.is_empty()
                    || name.len() > 253
                    || !name.is_ascii()
                    || name.ends_with('.')
                    || url::Host::parse(name).ok().as_ref() != Some(&host)
                {
                    return Err(invalid());
                }
                (name.as_str(), None, name.capacity(), false)
            }
            url::Host::Ipv4(address) => {
                let text = address.to_string();
                ("", Some(text), 0, true)
            }
            url::Host::Ipv6(address) => {
                let text = format!("[{address}]");
                ("", Some(text), 0, true)
            }
        };
        let ip_name = match &host {
            url::Host::Ipv4(a) => Some(a.to_string()),
            url::Host::Ipv6(a) => Some(a.to_string()),
            _ => None,
        };
        let name = ip_name.as_deref().unwrap_or(name);
        let authority_name = authority_name.as_deref().unwrap_or(name);
        let mut digits = 1;
        let mut number = port;
        while number >= 10 {
            number /= 10;
            digits += 1;
        }
        let authority_len = sum(&[
            authority_name.len(),
            if port == 443 { 0 } else { 1 + digits },
        ])?;
        // Measured own buffers and defined fixed/opaque metadata charges. The
        // provider's internal allocations are not asserted to equal these sizes.
        reservation.grow(Retained {
            config: sum(&[
                size_of::<Self>(),
                size_of::<RootCertStore>(),
                size_of::<ClientConfig>(),
                size_of::<rustls::crypto::CryptoProvider>(),
                host_capacity,
                name.len(),
                header_charge(authority_len)?,
                size_of::<Vec<u8>>(),
                8,
            ])?,
            ..Retained::default()
        })?;
        preflight_roots(&roots, &mut reservation)?;
        // Existing service syntax/policy checks run under the full reservation,
        // before normalizing or copying any trust material.
        let material = validate(&host)?;
        let roots = Arc::new(normalized(&roots, &mut reservation)?);
        let server_name_string =
            String::from_utf8(copied(name.as_bytes(), &mut reservation, false)?)
                .map_err(|_| invalid())?;
        let server_name = ServerName::try_from(server_name_string).map_err(|_| invalid())?;
        if expect_ip != matches!(server_name, ServerName::IpAddress(_)) {
            return Err(invalid());
        }
        let mut authority = Vec::new();
        allocation_point()?;
        authority
            .try_reserve_exact(authority_len)
            .map_err(|_| unavailable())?;
        reservation.grow(Retained {
            config: authority
                .capacity()
                .checked_sub(authority_len)
                .ok_or_else(invalid)?,
            ..Retained::default()
        })?;
        authority.extend_from_slice(authority_name.as_bytes());
        if port != 443 {
            authority.push(b':');
            authority.extend_from_slice(port.to_string().as_bytes());
        }
        let authority_claim = reservation.split(Retained {
            config: header_charge(authority.capacity())?,
            ..Retained::default()
        })?;
        let authority = owned_header(authority, authority_claim)?;
        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let mut config = ClientConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
            .map_err(|_| invalid())?
            .with_root_certificates(roots)
            .with_no_client_auth();
        let alpn = copied(b"http/1.1", &mut reservation, false)?;
        let mut protocols = Vec::new();
        allocation_point()?;
        protocols.try_reserve_exact(1).map_err(|_| unavailable())?;
        reservation.grow(Retained {
            config: slots::<Vec<u8>>(protocols.capacity())?
                .checked_sub(size_of::<Vec<u8>>())
                .ok_or_else(invalid)?,
            ..Retained::default()
        })?;
        protocols.push(alpn);
        config.alpn_protocols = protocols;
        config.enable_sni = true;
        config.enable_early_data = false;
        config.resumption = rustls::client::Resumption::disabled();
        Ok((
            Arc::new(Self {
                config: Arc::new(config),
                host,
                server_name,
                authority,
                identity,
                port,
                _claim: reservation,
            }),
            material,
        ))
    }
    pub(in crate::http_client) fn identity(&self) -> ServiceIdentity {
        self.identity
    }
    pub(in crate::http_client) fn port(&self) -> u16 {
        self.port
    }
    pub(in crate::http_client) fn host(&self) -> &url::Host<String> {
        &self.host
    }
    pub(in crate::http_client) fn authority(&self) -> HeaderValue {
        self.authority.clone()
    }
    pub(in crate::http_client) fn server_name(&self) -> ServerName<'static> {
        self.server_name.clone()
    }
    pub(in crate::http_client) fn connector(&self) -> tokio_rustls::TlsConnector {
        tokio_rustls::TlsConnector::from(Arc::clone(&self.config))
    }
    pub(in crate::http_client) fn budget(&self) -> &Arc<RetentionBudget> {
        self._claim.budget()
    }
    #[cfg(test)]
    pub(in crate::http_client) fn for_test(
        catalog: &StartupCatalog,
        identity: ServiceIdentity,
        host: url::Host<String>,
        port: u16,
        roots: RootCertStore,
    ) -> Arc<Self> {
        Self::checked(&catalog.budget, identity, host, port, roots, 0, |_| Ok(()))
            .unwrap()
            .0
    }
}

#[cfg(test)]
#[path = "retention_tests.rs"]
mod tests;
