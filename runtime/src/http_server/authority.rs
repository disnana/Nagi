//! Checked startup authority and transport-peer configuration. Header values
//! never choose a trust profile or an external origin.
use super::{positive_size, Error, HeaderMap, Options, Status, Uri};
use axum::http::{uri::Authority, Version};
use std::{collections::HashSet, fmt, net::IpAddr};

#[derive(Debug, PartialEq, Eq, Hash)]
enum Host {
    Dns(String),
    Ip(IpAddr),
}
#[derive(Debug, PartialEq, Eq, Hash)]
struct WireAuthority {
    host: Host,
    port: Option<u16>,
}
impl WireAuthority {
    fn checked(input: &str) -> Result<Self, Error> {
        if input.is_empty()
            || !input.is_ascii()
            || input
                .bytes()
                .any(|b| b <= b' ' || matches!(b, 127 | b'@' | b'%' | b','))
        {
            return Err(Error::invalid("invalid HTTP authority"));
        }
        let parsed: Authority = input
            .parse()
            .map_err(|_| Error::invalid("invalid HTTP authority"))?;
        let host = parsed.host();
        // Authority permits userinfo, and port() treats some malformed ports
        // as absent. Check the entire suffix instead of accepting that loss.
        let suffix = input
            .strip_prefix(host)
            .ok_or_else(|| Error::invalid("invalid HTTP authority"))?;
        let port = if suffix.is_empty() {
            None
        } else {
            let text = suffix
                .strip_prefix(':')
                .ok_or_else(|| Error::invalid("invalid HTTP authority port"))?;
            let port = text
                .parse::<u16>()
                .ok()
                .filter(|p| *p != 0 && p.to_string() == text)
                .ok_or_else(|| Error::invalid("invalid HTTP authority port"))?;
            if parsed.port_u16() != Some(port) {
                return Err(Error::invalid("invalid HTTP authority port"));
            }
            Some(port)
        };
        let host = if let Some(ip) = host.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            Host::Ip(IpAddr::V6(
                ip.parse()
                    .map_err(|_| Error::invalid("invalid IPv6 authority"))?,
            ))
        } else if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
            Host::Ip(IpAddr::V4(ip))
        } else {
            // ASCII DNS labels only. IDNA equivalence/Unicode conversion is
            // deliberately delegated to deployment tooling, not reimplemented.
            if host.len() > 253
                || host.bytes().all(|b| b.is_ascii_digit() || b == b'.')
                || !host.split('.').all(|label| {
                    !label.is_empty()
                        && label.len() <= 63
                        && label.as_bytes()[0].is_ascii_alphanumeric()
                        && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                        && label
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                })
            {
                return Err(Error::invalid("invalid ASCII DNS authority"));
            }
            Host::Dns(host.to_ascii_lowercase())
        };
        Ok(Self { host, port })
    }
}

// Distinct from wire authority: origin has an effective HTTPS port, whereas
// omitted and explicit wire ports are separate allowlist entries.
struct ExternalOrigin {
    _host: Host,
    _effective_port: u16,
}
impl ExternalOrigin {
    fn checked(input: &str) -> Result<Self, Error> {
        let uri: Uri = input
            .parse()
            .map_err(|_| Error::invalid("invalid external HTTPS origin"))?;
        let authority = uri
            .authority()
            .ok_or_else(|| Error::invalid("invalid external HTTPS origin"))?;
        if uri.scheme_str() != Some("https")
            || input.strip_prefix("https://") != Some(authority.as_str())
        {
            return Err(Error::invalid(
                "external HTTPS origin must not contain a path or other components",
            ));
        }
        let wire = WireAuthority::checked(authority.as_str())?;
        Ok(Self {
            _host: wire.host,
            _effective_port: wire.port.unwrap_or(443),
        })
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
