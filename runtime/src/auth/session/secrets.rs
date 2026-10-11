//! Private D3 secret material. No public unchecked constructor or serialization API.
// Internal component classification; Store maps it into its stable Failure/Outcome.
pub(super) enum FailureKind {
    InvalidRequest,
    InvalidCredential,
    Unavailable,
}
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use cookie::{Cookie, SameSite};
use hyper::header::{HeaderName, HeaderValue};
use sha2::{Digest as _, Sha256};
#[cfg(test)]
#[path = "secrets_tests.rs"]
mod tests;
const MAX_NAME_BYTES: usize = 128;
const MAX_COOKIE_BYTES: usize = 8192;
const MAX_COOKIE_HEADERS: usize = 64;
const MAX_COOKIE_PAIRS: usize = 64;
const MAX_SET_COOKIE_BYTES: usize = 512;
pub(super) struct RawEntropy([u8; 32]);
pub(super) struct SessionId(String);
pub(super) struct Digest([u8; 32]);
pub(super) struct SetCookie(HeaderValue);
pub(super) struct CookiePolicy {
    name: String,
    same_site: SameSite,
}
// Common private primitive; production has only the OS provider call below.
fn fill_entropy(
    fill: impl FnOnce(&mut [u8]) -> Result<(), getrandom::Error>,
) -> Result<RawEntropy, FailureKind> {
    let mut raw = [0u8; 32];
    fill(&mut raw).map_err(|_| FailureKind::Unavailable)?;
    Ok(RawEntropy(raw))
}
impl RawEntropy {
    pub(super) fn generate() -> Result<Self, FailureKind> {
        fill_entropy(getrandom::fill)
    }
    pub(super) fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}
impl SessionId {
    pub(super) fn generate() -> Result<Self, FailureKind> {
        Ok(Self::from_entropy(RawEntropy::generate()?))
    }
    fn from_entropy(raw: RawEntropy) -> Self {
        Self(URL_SAFE_NO_PAD.encode(raw.0))
    }
    #[cfg(test)]
    pub(super) fn test_from_entropy(raw: [u8; 32]) -> Self {
        Self::from_entropy(RawEntropy(raw))
    }
    fn parse(value: &str) -> Result<Self, FailureKind> {
        if value.len() != 43 || !value.is_ascii() {
            return Err(FailureKind::InvalidCredential);
        }
        let mut raw = [0u8; 32];
        let length = URL_SAFE_NO_PAD
            .decode_slice(value, &mut raw)
            .map_err(|_| FailureKind::InvalidCredential)?;
        if length != 32 || URL_SAFE_NO_PAD.encode(raw) != value {
            return Err(FailureKind::InvalidCredential);
        }
        Ok(Self(value.to_owned()))
    }
    pub(super) fn digest(&self) -> Digest {
        Digest(Sha256::digest(self.0.as_bytes()).into())
    }
}
impl Digest {
    pub(super) fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}
impl SetCookie {
    pub(super) fn into_header(self) -> HeaderValue {
        self.0
    }
}
impl CookiePolicy {
    pub(super) fn new(name: &str, same_site: SameSite) -> Result<Self, FailureKind> {
        if name.len() > MAX_NAME_BYTES
            || !name
                .strip_prefix("__Host-")
                .is_some_and(|suffix| !suffix.is_empty())
            || HeaderName::from_bytes(name.as_bytes()).is_err()
            || same_site == SameSite::None
        {
            // RFC101 None is retained; SF03 guards must be connected before use.
            return Err(FailureKind::InvalidRequest);
        }
        Ok(Self {
            name: name.to_owned(),
            same_site,
        })
    }
    pub(super) fn name(&self) -> &str {
        &self.name
    }
    pub(super) fn same_site_code(&self) -> i64 {
        match self.same_site {
            SameSite::Strict => 1,
            SameSite::Lax => 2,
            SameSite::None => 3,
        }
    }
    fn serialize(&self, value: String, removal: bool) -> Result<SetCookie, FailureKind> {
        let mut cookie = Cookie::build((self.name.clone(), value))
            .same_site(self.same_site)
            .secure(true)
            .http_only(true)
            .path("/")
            .build();
        if removal {
            cookie.set_max_age(cookie::time::Duration::ZERO);
            cookie.set_expires(cookie::time::OffsetDateTime::UNIX_EPOCH);
        }
        let wire = cookie.to_string();
        if wire.len() > MAX_SET_COOKIE_BYTES {
            return Err(FailureKind::Unavailable);
        }
        let mut header = HeaderValue::from_str(&wire).map_err(|_| FailureKind::Unavailable)?;
        header.set_sensitive(true);
        Ok(SetCookie(header))
    }
    pub(super) fn issue(&self, id: SessionId) -> Result<SetCookie, FailureKind> {
        self.serialize(id.0, false)
    }
    pub(super) fn remove(&self) -> Result<SetCookie, FailureKind> {
        self.serialize(String::new(), true)
    }
    pub(super) fn read(
        &self,
        headers: &[&str],
        authorization_present: bool,
    ) -> Result<SessionId, FailureKind> {
        if authorization_present || headers.len() > MAX_COOKIE_HEADERS {
            return Err(FailureKind::InvalidRequest);
        }
        let bytes = headers
            .iter()
            .try_fold(0usize, |n, h| n.checked_add(h.len()))
            .ok_or(FailureKind::InvalidRequest)?;
        if bytes > MAX_COOKIE_BYTES {
            return Err(FailureKind::InvalidRequest);
        }
        let mut pair_count = 0;
        let mut selected = None;
        for header in headers {
            for pair in Cookie::split_parse(*header) {
                pair_count += 1;
                if pair_count > MAX_COOKIE_PAIRS {
                    return Err(FailureKind::InvalidRequest);
                }
                let pair = pair.map_err(|_| FailureKind::InvalidRequest)?;
                if pair.name() == self.name {
                    if selected.is_some() {
                        return Err(FailureKind::InvalidRequest);
                    }
                    // Retain classification, then inspect every remaining pair.
                    // Malformed/duplicate input wins over invalid ID (400 vs401).
                    selected = Some(SessionId::parse(pair.value()));
                }
            }
        }
        selected.unwrap_or(Err(FailureKind::InvalidCredential))
    }
}
