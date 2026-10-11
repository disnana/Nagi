//! Checked startup authority and transport-peer configuration. Header values
//! never choose a trust profile or an external origin.
use super::origin::{Origin, WireAuthority};
use super::{positive_size, Error, HeaderMap, Options, Status, Uri};
use axum::http::Version;
use std::{collections::HashSet, fmt, net::IpAddr};

// HTTPS-only security origin wrapper. CORS cannot weaken this invariant.
#[derive(PartialEq, Eq)]
pub(crate) struct ExternalOrigin(Origin);
impl ExternalOrigin {
    fn serialized(&self) -> Result<String, Error> {
        let value = self.0.serialized();
        if !self.matches(&value) {
            return Err(Error::invalid(
                "checked external origin could not be serialized",
            ));
        }
        Ok(value)
    }
    pub(crate) fn matches_serialized(bound: &str, input: &str) -> bool {
        Self::checked(bound).is_ok_and(|origin| origin.matches(input))
    }
    pub(crate) fn is_canonical(input: &str) -> bool {
        Self::checked(input).is_ok_and(|origin| origin.serialized().is_ok_and(|s| s == input))
    }
    pub(crate) fn matches(&self, input: &str) -> bool {
        Self::checked(input).is_ok_and(|other| other == *self)
    }
    fn checked(input: &str) -> Result<Self, Error> {
        let origin = Origin::checked(input)?;
        if !origin.is_https() {
            return Err(Error::invalid("external origin requires HTTPS"));
        }
        Ok(Self(origin))
    }
}

pub(super) struct Config {
    _origin: ExternalOrigin,
    wire: HashSet<WireAuthority>,
    peers: Option<HashSet<IpAddr>>,
    entry_limit: usize,
    byte_limit: usize,
}
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CheckedAuthority")
            .field("wire_count", &self.wire.len())
            .field("proxy_peer_count", &self.peers.as_ref().map(HashSet::len))
            .finish_non_exhaustive()
    }
}
fn bounded(values: &[String], count: usize, bytes: usize) -> Result<(), Error> {
    if values.is_empty() || values.len() > count {
        return Err(Error::invalid(
            "HTTP authority configuration exceeds its entry limit",
        ));
    }
    values.iter().try_fold(0usize, |total, s| {
        total
            .checked_add(s.len())
            .filter(|n| *n <= bytes)
            .ok_or_else(|| Error::invalid("HTTP authority configuration exceeds its byte limit"))
    })?;
    Ok(())
}

/// The origin and the accepted wire authorities are separate startup facts.
pub fn authority(
    mut options: Options,
    external_origin: &str,
    authorities: Vec<String>,
    entry_limit: i64,
    byte_limit: i64,
) -> Result<Options, Error> {
    if options.authority.is_some() {
        return Err(Error::invalid(
            "HTTP authority configuration is already sealed",
        ));
    }
    let entry_limit = positive_size(entry_limit, isize::MAX as usize, "authority entries")?;
    let byte_limit = positive_size(byte_limit, isize::MAX as usize, "authority bytes")?;
    if external_origin.len() > byte_limit {
        return Err(Error::invalid("external origin exceeds its byte limit"));
    }
    bounded(&authorities, entry_limit, byte_limit)?;
    let origin = ExternalOrigin::checked(external_origin)?;
    let mut wire = HashSet::new();
    for value in authorities {
        if !wire.insert(WireAuthority::checked(&value)?) {
            return Err(Error::invalid("duplicate canonical HTTP authority"));
        }
    }
    options.authority = Some(Config {
        _origin: origin,
        wire,
        peers: None,
        entry_limit,
        byte_limit,
    });
    Ok(options)
}

/// Exact IP ACLs use the same finite startup count/byte ceilings. The peer is
/// supplied only by TcpListener::accept, never by Forwarded or application code.
pub fn trusted_proxy(mut options: Options, peer_ips: Vec<String>) -> Result<Options, Error> {
    let config = options
        .authority
        .as_mut()
        .ok_or_else(|| Error::invalid("configure HTTP authority before its proxy profile"))?;
    if config.peers.is_some() {
        return Err(Error::invalid("HTTP proxy configuration is already sealed"));
    }
    bounded(&peer_ips, config.entry_limit, config.byte_limit)?;
    let mut peers = HashSet::new();
    for value in peer_ips {
        let ip = canonical_peer(
            value
                .parse()
                .map_err(|_| Error::invalid("invalid proxy peer IP"))?,
        );
        if ip.is_unspecified() || ip.is_multicast() || !peers.insert(ip) {
            return Err(Error::invalid("invalid or duplicate proxy peer IP"));
        }
    }
    config.peers = Some(peers);
    Ok(options)
}
fn canonical_peer(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(ip) => ip
            .to_ipv4_mapped()
            .map(IpAddr::V4)
            .unwrap_or(IpAddr::V6(ip)),
        ip => ip,
    }
}
impl Config {
    pub(super) fn origin(&self) -> &ExternalOrigin {
        &self._origin
    }
    pub(super) fn check_safe_origin(&self, headers: &HeaderMap) -> Result<(), Status> {
        let mut origins = headers.get_all(axum::http::header::ORIGIN).iter();
        let Some(origin) = origins.next() else {
            return Ok(());
        };
        if origins.next().is_some() || origin.as_bytes().len() > self.byte_limit {
            return Err(Status::FORBIDDEN);
        }
        let origin = origin.to_str().map_err(|_| Status::FORBIDDEN)?;
        if !self._origin.matches(origin) {
            return Err(Status::FORBIDDEN);
        }
        Ok(())
    }
    pub(super) fn check_unsafe_origin(&self, headers: &HeaderMap) -> Result<(), Status> {
        if !headers.contains_key(axum::http::header::ORIGIN) {
            return Err(Status::FORBIDDEN);
        }
        self.check_safe_origin(headers)
    }
    pub(super) fn check(
        &self,
        version: Version,
        uri: &Uri,
        headers: &HeaderMap,
        peer: IpAddr,
    ) -> Result<(), Status> {
        if version == Version::HTTP_10 {
            return Err(Status::HTTP_VERSION_NOT_SUPPORTED);
        }
        if version != Version::HTTP_11
            || uri.scheme().is_some()
            || uri.authority().is_some()
            || !uri.path().starts_with('/')
            || self
                .peers
                .as_ref()
                .is_some_and(|peers| !peers.contains(&canonical_peer(peer)))
            || headers
                .keys()
                .any(|name| name == "forwarded" || name.as_str().starts_with("x-forwarded-"))
        {
            return Err(Status::BAD_REQUEST);
        }
        let mut hosts = headers.get_all(axum::http::header::HOST).iter();
        let host = hosts.next().ok_or(Status::BAD_REQUEST)?;
        if hosts.next().is_some() || host.as_bytes().len() > self.byte_limit {
            return Err(Status::BAD_REQUEST);
        }
        let host = host.to_str().map_err(|_| Status::BAD_REQUEST)?;
        let host = WireAuthority::checked(host).map_err(|_| Status::BAD_REQUEST)?;
        if !self.wire.contains(&host) {
            return Err(Status::BAD_REQUEST);
        }
        Ok(())
    }
}

pub(super) fn require_authority(options: &Options) -> Result<(), Error> {
    if options.authority.is_none() {
        return Err(Error::invalid(
            "HTTP authority must be explicitly configured before serving",
        ));
    }
    Ok(())
}

// Closed nonsecret startup serialization; never returns a Pool, path, handle,
// request credential or mutable authority profile.
pub(crate) fn checked_prelogin_origin(options: &Options) -> Result<String, Error> {
    options
        .authority
        .as_ref()
        .ok_or_else(|| {
            Error::invalid("HTTP authority must be explicitly configured before pre-login startup")
        })?
        ._origin
        .serialized()
}
