//! Closed BrowserLogin input/claim context. No public proof or material factory.
use super::*;
use hyper::header::{HeaderMap, HeaderValue, AUTHORIZATION, COOKIE, ORIGIN};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use std::{
    collections::HashSet,
    sync::{atomic::Ordering, Weak},
};

pub(crate) struct OwnerBinding {
    pub(super) lease: Arc<Lease>,
    cell: Weak<Mutex<Cell>>,
}
pub(crate) struct LoginInput {
    input: ClaimInput,
}
pub(crate) struct ClaimContext {
    consumed: Consumed,
    binding: OwnerBinding,
}
impl ClaimContext {
    pub(crate) fn lease(&self) -> Arc<Lease> {
        Arc::clone(&self.binding.lease)
    }
    pub(crate) async fn verify<F, Fut>(self, verifier: F) -> Result<VerifiedIdentity, Failure>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<VerifiedIdentity, super::super::Failure>>,
    {
        self.consumed.verify(&self.binding, verifier).await
    }
}
impl RequestOwner {
    pub(crate) fn binding(&self) -> OwnerBinding {
        OwnerBinding {
            lease: Arc::clone(&self.lease.lease),
            cell: Arc::downgrade(&self.cell),
        }
    }
    pub(crate) fn request_until(&self) -> Result<Instant, Failure> {
        Ok(self
            .lease
            .lease
            .checked_gate()
            .map_err(Failure::authority)?
            .expires_at)
    }
    pub(crate) fn lease_owner(&self) -> &LeaseOwner {
        &self.lease
    }
    pub(crate) fn mark_consumed(&self) {
        self.consumed.store(true, Ordering::Release);
    }
    pub(crate) fn was_consumed(&self) -> bool {
        self.consumed.load(Ordering::Acquire)
    }
}
impl OwnerBinding {
    pub(super) fn present(&self) -> Result<(), Failure> {
        if self.cell.strong_count() == 0 {
            return Err(Failure::authority(super::super::expired()));
        }
        self.lease.validate().map_err(Failure::authority)
    }
}
impl Failure {
    pub(crate) fn login_status(&self) -> crate::http_server::Status {
        use crate::http_server::Status;
        match (&self.origin, self.kind) {
            (FailureOrigin::Verifier, FailureKind::InvalidCredential | FailureKind::Expired) => {
                Status::UNAUTHORIZED
            }
            (FailureOrigin::Authority, FailureKind::Expired) => Status::GATEWAY_TIMEOUT,
            (FailureOrigin::Core, FailureKind::Expired) if !self.business_committed => {
                Status::FORBIDDEN
            }
            _ => self.http_status(),
        }
    }
    pub(crate) fn login_challenge(&self) -> Option<HeaderValue> {
        (self.login_status() == crate::http_server::Status::UNAUTHORIZED)
            .then(|| HeaderValue::from_static("Bearer"))
    }
    pub(crate) fn from_authority(error: super::super::Failure) -> Self {
        Self::authority(error)
    }
}
impl Pool {
    // Closed test-only setup: real bootstrap material remains inside PreLogin.
    // The returned owner/context are a one-shot pair from actual claim_http.
    #[cfg(test)]
    pub(crate) async fn claim_stage_for_test(&self) -> Result<(HttpOwner, ClaimContext), Failure> {
        let bootstrap = HttpOwner::new(
            LeaseOwner::new(Instant::now() + Duration::from_secs(10))
                .map_err(Failure::authority)?,
        );
        self.bootstrap(&bootstrap, None).await?;
        let input = {
            let cell = bootstrap.cell.lock().map_err(|_| Failure::unavailable())?;
            let pair = cell.pair.as_ref().ok_or_else(Failure::unavailable)?;
            LoginInput {
                input: ClaimInput::parse(&pair.handle, &pair.token)?,
            }
        };
        let owner = HttpOwner::new(
            LeaseOwner::new(Instant::now() + Duration::from_secs(10))
                .map_err(Failure::authority)?,
        );
        let context = self.claim_http(&owner, input).await?;
        Ok((owner, context))
    }
    pub(crate) fn login_input(&self, headers: &HeaderMap) -> Result<LoginInput, Failure> {
        // Inspect all Cookie pairs and ambiguous fields before classifying a
        // missing/bad credential. No Cookie is an identity fallback.
        let single = |name: &str| -> Result<Option<&HeaderValue>, Failure> {
            let mut values = headers.get_all(name).iter();
            let first = values.next();
            if values.next().is_some() {
                return Err(Failure::invalid());
            }
            Ok(first)
        };
        let authorization = single(AUTHORIZATION.as_str())?;
        let origin = single(ORIGIN.as_str())?;
        let token = single("x-csrf-token")?;
        let mut bytes = 0usize;
        let mut count = 0usize;
        let mut pairs = 0usize;
        let mut names = HashSet::new();
        let mut handle = None;
        for value in headers.get_all(COOKIE).iter() {
            count += 1;
            bytes = bytes
                .checked_add(value.as_bytes().len())
                .ok_or_else(Failure::invalid)?;
            if count > 64 || bytes > 8192 {
                return Err(Failure::invalid());
            }
            let text = value.to_str().map_err(|_| Failure::invalid())?;
            for pair in cookie::Cookie::split_parse(text) {
                pairs += 1;
                if pairs > 64 {
                    return Err(Failure::invalid());
                }
                let pair = pair.map_err(|_| Failure::invalid())?;
                if !names.insert(pair.name().to_owned())
                    || pair.name() == self.context.cookie_name()
                {
                    return Err(Failure::invalid());
                }
                if pair.name() == self.cookie.name {
                    handle = Some(pair.value().to_owned());
                }
            }
        }
        let value =
            authorization.ok_or_else(|| Failure::verifier(super::super::invalid_credential()))?;
        let value = value.to_str().map_err(|_| Failure::invalid())?;
        let Some((scheme, bearer)) = value.split_once(' ') else {
            return Err(Failure::verifier(super::super::invalid_credential()));
        };
        if !scheme.eq_ignore_ascii_case("Bearer")
            || bearer.is_empty()
            || bearer.bytes().any(|b| b <= b' ' || b == 127)
        {
            return Err(Failure::verifier(super::super::invalid_credential()));
        }
        let origin = origin
            .ok_or_else(Failure::denied)?
            .to_str()
            .map_err(|_| Failure::denied())?;
        if origin.len() > 8192
            || !crate::http_server::BoundOrigin::matches_serialized(&self.origin, origin)
        {
            return Err(Failure::denied());
        }
        let token = token
            .ok_or_else(Failure::denied)?
            .to_str()
            .map_err(|_| Failure::denied())?;
        ClaimInput::parse(handle.as_deref().ok_or_else(Failure::denied)?, token)
            .map(|input| LoginInput { input })
    }
    pub(crate) fn login_json(&self, body: &[u8]) -> Result<(), Failure> {
        let mut limits = Limits {
            depth: 0,
            nodes: 0,
            max_depth: self.options.max_json_depth as usize,
            max_nodes: self.options.max_json_nodes as usize,
        };
        let mut parser = serde_json::Deserializer::from_slice(body);
        Seed(&mut limits)
            .deserialize(&mut parser)
            .map_err(|_| Failure::invalid())?;
        parser.end().map_err(|_| Failure::invalid())
    }
    pub(crate) async fn claim_http(
        &self,
        owner: &RequestOwner,
        input: LoginInput,
    ) -> Result<ClaimContext, Failure> {
        // Claim may tighten the real gate after loading the ready row. Re-read
        // that gate after each poll, so a pending native await cannot keep the
        // original longer security timer. No detached task or fresh budget.
        let mut claim = std::pin::pin!(self.claim(owner, input.input));
        let mut timer = std::pin::pin!(tokio::time::sleep_until(tokio::time::Instant::from_std(
            owner.request_until()?
        )));
        let consumed = std::future::poll_fn(|cx| {
            let result = claim.as_mut().poll(cx);
            let until = match owner.request_until() {
                Ok(until) => until,
                Err(error) => {
                    return std::task::Poll::Ready(Err(if owner.was_consumed() {
                        error.after_commit()
                    } else {
                        error
                    }))
                }
            };
            timer.as_mut().reset(tokio::time::Instant::from_std(until));
            if timer.as_mut().poll(cx).is_ready() {
                let error = Failure::authority(super::super::expired());
                return std::task::Poll::Ready(Err(if owner.was_consumed() {
                    error.after_commit()
                } else {
                    error
                }));
            }
            result
        })
        .await?;
        Ok(ClaimContext {
            consumed,
            binding: owner.binding(),
        })
    }
    pub(crate) fn deletion_cookie(&self) -> Result<HeaderValue, Failure> {
        let site = if self.cookie.same_site == session::SameSite::Strict {
            cookie::SameSite::Strict
        } else {
            cookie::SameSite::Lax
        };
        let value = cookie::Cookie::build((self.cookie.name.clone(), String::new()))
            .secure(true)
            .http_only(true)
            .path("/")
            .same_site(site)
            .max_age(cookie::time::Duration::ZERO)
            .build()
            .to_string();
        if value.len() > 512 {
            return Err(Failure::wire_internal());
        }
        let mut value = HeaderValue::from_str(&value).map_err(|_| Failure::wire_internal())?;
        value.set_sensitive(true);
        Ok(value)
    }
}
struct Limits {
    depth: usize,
    nodes: usize,
    max_depth: usize,
    max_nodes: usize,
}
struct Seed<'a>(&'a mut Limits);
struct Json<'a>(&'a mut Limits);
impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, parser: D) -> Result<(), D::Error> {
        self.0.nodes = self
            .0
            .nodes
            .checked_add(1)
            .ok_or_else(|| de::Error::custom("JSON node limit"))?;
        if self.0.nodes > self.0.max_nodes {
            return Err(de::Error::custom("JSON node limit"));
        }
        parser.deserialize_any(Json(self.0))
    }
}
impl<'de> Visitor<'de> for Json<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded JSON value")
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E: de::Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        self.0.depth += 1;
        if self.0.depth > self.0.max_depth {
            return Err(de::Error::custom("JSON depth limit"));
        }
        while seq.next_element_seed(Seed(self.0))?.is_some() {}
        self.0.depth -= 1;
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        self.0.depth += 1;
        if self.0.depth > self.0.max_depth {
            return Err(de::Error::custom("JSON depth limit"));
        }
        let mut keys = HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key) {
                return Err(de::Error::custom("duplicate JSON member"));
            }
            map.next_value_seed(Seed(self.0))?;
        }
        self.0.depth -= 1;
        Ok(())
    }
}
