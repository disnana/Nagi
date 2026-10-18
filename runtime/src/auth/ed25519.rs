//! Bounded Ed25519 bearer verification for trusted native HTTP adapters.
//! Authentication framing belongs to JWT11 public decode, never this preflight.
use super::{Failure, VerifiedIdentity};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::{
    collections::BTreeMap,
    fmt,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[path = "ed25519/provider_binding.rs"]
mod provider_binding;

/// Explicit finite limits for one verifier. No permissive default is provided.
pub struct Limits {
    pub token_bytes: usize,
    pub header_bytes: usize,
    pub claim_bytes: usize,
    pub depth: usize,
    pub audiences: usize,
    pub kid_bytes: usize,
    pub keys: usize,
    pub max_ttl: Duration,
}
struct Startup {
    issuer: String,
    audience: String,
    keys: Vec<(String, Vec<u8>)>,
    limits: Limits,
}
/// A prepared verifier for a trusted named native callback.
pub struct Verifier {
    issuer: String,
    audience: String,
    keys: BTreeMap<String, DecodingKey>,
    limits: Limits,
    validation: Validation,
    _provider: &'static provider_binding::ProviderProof,
}
struct ClockSample {
    before: Instant,
    utc: Duration,
    after: Instant,
}
fn clock() -> Result<ClockSample, Failure> {
    let before = Instant::now();
    let utc = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Failure::unavailable())?;
    let after = Instant::now();
    Ok(ClockSample { before, utc, after })
}
impl Verifier {
    /// Prepare one issuer and audience with fixed-size raw Ed25519 public keys.
    /// Invalid finite inputs are rejected before any process-global provider install.
    pub fn checked(
        issuer: String,
        audience: String,
        keys: Vec<(String, [u8; 32])>,
        limits: Limits,
    ) -> Result<Arc<Verifier>, Failure> {
        if keys.is_empty() || keys.len() > limits.keys {
            return Err(Failure::invalid_request());
        }
        Self::checked_startup(Startup {
            issuer,
            audience,
            keys: keys
                .into_iter()
                .map(|(kid, raw)| (kid, raw.to_vec()))
                .collect(),
            limits,
        })
    }

    // Retain the bounded raw-byte preparation path for internal startup and
    // malformed-key rejection fixtures; it is not a public constructor.
    fn checked_startup(input: Startup) -> Result<Arc<Self>, Failure> {
        let l = &input.limits;
        if [
            l.token_bytes,
            l.header_bytes,
            l.claim_bytes,
            l.depth,
            l.audiences,
            l.kid_bytes,
            l.keys,
        ]
        .contains(&0)
            || l.depth > 128
            || l.header_bytes > l.token_bytes
            || l.claim_bytes > l.token_bytes
            || l.token_bytes.checked_mul(3).is_none()
            || l.kid_bytes
                .checked_add(32)
                .and_then(|n| n.checked_mul(l.keys))
                .is_none()
            || l.max_ttl.is_zero()
            || Instant::now().checked_add(l.max_ttl).is_none()
            || input.issuer.is_empty()
            || input.audience.is_empty()
            || input.issuer.len() > l.claim_bytes
            || input.audience.len() > l.claim_bytes
            || input.keys.is_empty()
            || input.keys.len() > l.keys
        {
            return Err(Failure::invalid_request());
        }
        let mut keys = BTreeMap::new();
        // Complete all per-config finite/raw-key preparation BEFORE touching JWT global.
        for (kid, raw) in input.keys {
            if kid.is_empty()
                || kid.len() > l.kid_bytes
                || raw.len() != 32
                || keys.contains_key(&kid)
            {
                return Err(Failure::invalid_request());
            }
            aws_lc_rs::signature::ParsedPublicKey::new(&aws_lc_rs::signature::ED25519, &raw)
                .map_err(|_| Failure::invalid_request())?;
            keys.insert(kid, DecodingKey::from_ed_der(&raw));
        }
        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.leeway = 0;
        validation.reject_tokens_expiring_in_less_than = 0;
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        validation.set_issuer(&[input.issuer.as_str()]);
        validation.set_audience(&[input.audience.as_str()]);
        // Strict signed integer/fractional clock decisions below are mandatory.
        // JWT's independent coarse clock / float rounding must not decide G3 time.
        validation.validate_exp = false;
        validation.validate_nbf = false;
        validation.validate_aud = true;
        let proof =
            provider_binding::require_provider_proof().map_err(|_| Failure::unavailable())?;
        Ok(Arc::new(Self {
            issuer: input.issuer,
            audience: input.audience,
            keys,
            limits: input.limits,
            validation,
            _provider: proof,
        }))
    }
    /// Verify the original borrowed Authorization bearer value in a request.
    pub fn verify_request(
        &self,
        request: &crate::http_server::Request,
    ) -> Result<VerifiedIdentity, Failure> {
        // Borrow immutable admitted headers; no token is exposed by a public API.
        self.verify(crate::http_server::bearer_token(request)?)
    }
    fn verify(&self, token: &str) -> Result<VerifiedIdentity, Failure> {
        self.verify_with_clock(token, clock)
    }
    fn verify_with_clock(
        &self,
        token: &str,
        sample: impl FnOnce() -> Result<ClockSample, Failure>,
    ) -> Result<VerifiedIdentity, Failure> {
        let key = self.preflight(token)?;
        // Same original immutable compact token. No reconstruction, insecure_decode,
        // direct verifier factory, request-time install, header URL or key discovery.
        let signed = jsonwebtoken::decode::<StrictSignedClaims>(token, key, &self.validation)
            .map_err(|_| Failure::invalid_credential())?
            .claims;
        self.validate_signed(signed, sample()?)
    }
    // Reject-only framing/serde/shape bounds. Success is NOT an identity/proof.
    fn preflight(&self, token: &str) -> Result<&DecodingKey, Failure> {
        if token.len() > self.limits.token_bytes {
            return Err(Failure::invalid_credential());
        }
        let mut parts = token.split('.');
        let header = parts.next().ok_or_else(Failure::invalid_credential)?;
        let claims = parts.next().ok_or_else(Failure::invalid_credential)?;
        let signature = parts.next().ok_or_else(Failure::invalid_credential)?;
        if parts.next().is_some() || signature.len() != 86 {
            return Err(Failure::invalid_credential());
        }
        let header_bytes = segment(header, self.limits.header_bytes)?;
        let claim_bytes = segment(claims, self.limits.claim_bytes)?;
        if segment(signature, 64)?.len() != 64 {
            return Err(Failure::invalid_credential());
        }
        let header = object(parse(&header_bytes, self.limits.depth)?)?;
        if header
            .keys()
            .any(|key| !matches!(key.as_str(), "alg" | "typ" | "kid"))
            || string(header.get("alg"))? != "EdDSA"
        {
            return Err(Failure::invalid_credential());
        }
        if let Some(typ) = header.get("typ") {
            if string(Some(typ))? != "JWT" {
                return Err(Failure::invalid_credential());
            }
        }
        let key = match header.get("kid") {
            Some(kid) => {
                let kid = string(Some(kid))?;
                if kid.len() > self.limits.kid_bytes {
                    return Err(Failure::invalid_credential());
                }
                self.keys.get(kid).ok_or_else(Failure::invalid_credential)?
            }
            None if self.keys.len() == 1 => self
                .keys
                .values()
                .next()
                .ok_or_else(Failure::invalid_credential)?,
            None => return Err(Failure::invalid_credential()),
        };
        let claims = strict_fields(object(parse(&claim_bytes, self.limits.depth)?)?)?;
        self.validate_non_time(&claims)?;
        Ok(key)
    }
    fn validate_non_time(&self, claims: &StrictSignedClaims) -> Result<(), Failure> {
        if claims.issuer != self.issuer
            || claims.audience.is_empty()
            || claims.audience.len() > self.limits.audiences
            || !claims.audience.iter().any(|a| a == &self.audience)
        {
            return Err(Failure::invalid_credential());
        }
        Ok(())
    }
    fn validate_signed(
        &self,
        claims: StrictSignedClaims,
        sample: ClockSample,
    ) -> Result<VerifiedIdentity, Failure> {
        // This repeats semantic validation on signature-success typed claims.
        self.validate_non_time(&claims)?;
        if sample.before > sample.after || sample.after > Instant::now() {
            return Err(Failure::unavailable());
        }
        let expiry = Duration::from_secs(claims.exp);
        UNIX_EPOCH
            .checked_add(expiry)
            .ok_or_else(Failure::invalid_credential)?;
        if sample.utc >= expiry
            || claims
                .nbf
                .is_some_and(|n| sample.utc < Duration::from_secs(n))
        {
            return Err(Failure::invalid_credential());
        }
        let remaining = expiry
            .checked_sub(sample.utc)
            .ok_or_else(Failure::invalid_credential)?;
        if remaining.is_zero() || remaining > self.limits.max_ttl {
            return Err(Failure::invalid_credential());
        }
        let expires_at = sample
            .before
            .checked_add(remaining)
            .ok_or_else(Failure::invalid_credential)?;
        // from_verified performs the final live-deadline check. Request/lease budget
        // is never substituted for this original credential absolute deadline.
        // SF01 dispatcher alone owns request deadline precedence/cancel/join/504.
        VerifiedIdentity::from_verified(claims.subject, expires_at)
    }
}
struct StrictSignedClaims {
    exp: u64,
    nbf: Option<u64>,
    issuer: String,
    audience: Vec<String>,
    subject: i64,
}
impl<'de> Deserialize<'de> for StrictSignedClaims {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // Configured byte/depth limits already rejected the SAME immutable bytes.
        // This signature-success parse still rejects all duplicate subtrees/shape.
        let value = Seed { remaining: 128 }.deserialize(d)?;
        object(value)
            .and_then(strict_fields)
            .map_err(|_| de::Error::custom("invalid signed claims"))
    }
}
fn strict_fields(claims: BTreeMap<String, Json>) -> Result<StrictSignedClaims, Failure> {
    let exp = integer(claims.get("exp"))?;
    let nbf = claims.get("nbf").map(|x| integer(Some(x))).transpose()?;
    let issuer = string(claims.get("iss"))?.to_owned();
    let subject = string(claims.get("sub"))?;
    let parsed = subject
        .parse::<i64>()
        .map_err(|_| Failure::invalid_credential())?;
    if parsed.to_string() != subject {
        return Err(Failure::invalid_credential());
    }
    let audience = match claims.get("aud") {
        Some(Json::String(aud)) => vec![aud.clone()],
        Some(Json::Array(aud)) if !aud.is_empty() => aud
            .iter()
            .map(|a| string(Some(a)).map(str::to_owned))
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err(Failure::invalid_credential()),
    };
    Ok(StrictSignedClaims {
        exp,
        nbf,
        issuer,
        audience,
        subject: parsed,
    })
}
fn segment(text: &str, max: usize) -> Result<Vec<u8>, Failure> {
    if text.is_empty()
        || text
            .len()
            .checked_mul(3)
            .ok_or_else(Failure::invalid_credential)?
            / 4
            > max
    {
        return Err(Failure::invalid_credential());
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(text)
        .map_err(|_| Failure::invalid_credential())?;
    if bytes.len() > max || URL_SAFE_NO_PAD.encode(&bytes) != text {
        return Err(Failure::invalid_credential());
    }
    Ok(bytes)
}
enum Json {
    Null,
    Bool,
    UInt(u64),
    Negative,
    Float,
    String(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}
fn string(value: Option<&Json>) -> Result<&str, Failure> {
    if let Some(Json::String(value)) = value {
        Ok(value)
    } else {
        Err(Failure::invalid_credential())
    }
}
fn integer(value: Option<&Json>) -> Result<u64, Failure> {
    if let Some(Json::UInt(value)) = value {
        Ok(*value)
    } else {
        Err(Failure::invalid_credential())
    }
}
fn object(value: Json) -> Result<BTreeMap<String, Json>, Failure> {
    if let Json::Object(value) = value {
        Ok(value)
    } else {
        Err(Failure::invalid_credential())
    }
}
struct Seed {
    remaining: usize,
}
impl<'de> DeserializeSeed<'de> for Seed {
    type Value = Json;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Json, D::Error> {
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Seed {
    type Value = Json;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded JSON value")
    }
    fn visit_unit<E: de::Error>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Json, E> {
        Ok(Json::Bool)
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Json, E> {
        Ok(Json::UInt(v))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Json, E> {
        Ok(if v < 0 {
            Json::Negative
        } else {
            Json::UInt(v as u64)
        })
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Json, E> {
        Ok(Json::Float)
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Json, E> {
        Ok(Json::String(v.to_owned()))
    }
    fn visit_string<E: de::Error>(self, v: String) -> Result<Json, E> {
        Ok(Json::String(v))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
        let remaining = self
            .remaining
            .checked_sub(1)
            .ok_or_else(|| de::Error::custom("depth"))?;
        let mut values = Vec::new();
        while let Some(value) = seq.next_element_seed(Seed { remaining })? {
            values.push(value);
        }
        Ok(Json::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
        let remaining = self
            .remaining
            .checked_sub(1)
            .ok_or_else(|| de::Error::custom("depth"))?;
        let mut values = BTreeMap::new();
        while let Some(key) = map.next_key::<String>()? {
            let value = map.next_value_seed(Seed { remaining })?;
            // Keys have already been JSON-decoded by serde, including escapes.
            // Unknown subtrees use this same visitor, so they cannot hide duplicates.
            if values.insert(key, value).is_some() {
                return Err(de::Error::custom("duplicate member"));
            }
        }
        Ok(Json::Object(values))
    }
}
fn parse(bytes: &[u8], depth: usize) -> Result<Json, Failure> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = Seed { remaining: depth }
        .deserialize(&mut deserializer)
        .map_err(|_| Failure::invalid_credential())?;
    deserializer
        .end()
        .map_err(|_| Failure::invalid_credential())?;
    Ok(value)
}
#[cfg(test)]
#[path = "ed25519/provider_process_tests.rs"]
mod provider_process_tests;
#[cfg(test)]
#[path = "ed25519/ed25519_tests.rs"]
mod tests;
