//! Shared checked ASCII origin grammar; wire authority retains omitted/explicit ports.
use super::{Error, Uri};
use axum::http::uri::Authority;
use std::net::IpAddr;
#[derive(Debug, PartialEq, Eq, Hash)]
enum Host {
    Dns(String),
    Ip(IpAddr),
}
#[derive(Debug, PartialEq, Eq, Hash)]
pub(super) struct WireAuthority {
    host: Host,
    port: Option<u16>,
}
impl WireAuthority {
    pub(super) fn checked(input: &str) -> Result<Self, Error> {
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

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Origin {
    https: bool,
    host: Host,
    effective_port: u16,
}
impl Origin {
    pub(super) fn checked(input: &str) -> Result<Self, Error> {
        let uri: Uri = input
            .parse()
            .map_err(|_| Error::invalid("invalid HTTP origin"))?;
        let authority = uri
            .authority()
            .ok_or_else(|| Error::invalid("invalid HTTP origin"))?;
        let https = match uri.scheme_str() {
            Some("https") if input.strip_prefix("https://") == Some(authority.as_str()) => true,
            Some("http") if input.strip_prefix("http://") == Some(authority.as_str()) => false,
            _ => {
                return Err(Error::invalid(
                    "origin must contain only an explicit http/https scheme and authority",
                ))
            }
        };
        let wire = WireAuthority::checked(authority.as_str())?;
        Ok(Self {
            https,
            host: wire.host,
            effective_port: wire.port.unwrap_or(if https { 443 } else { 80 }),
        })
    }
    pub(super) fn retained_bytes(&self) -> usize {
        match &self.host {
            Host::Dns(name) => name.capacity(),
            Host::Ip(_) => 0,
        }
    }
    pub(super) fn is_https(&self) -> bool {
        self.https
    }
    pub(super) fn serialized(&self) -> String {
        let scheme = if self.https { "https" } else { "http" };
        let host = match &self.host {
            Host::Dns(name) => name.clone(),
            Host::Ip(IpAddr::V4(ip)) => ip.to_string(),
            Host::Ip(IpAddr::V6(ip)) => format!("[{ip}]"),
        };
        if self.effective_port == if self.https { 443 } else { 80 } {
            format!("{scheme}://{host}")
        } else {
            format!("{scheme}://{host}:{}", self.effective_port)
        }
    }
}
