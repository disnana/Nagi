//! Closed bootstrap HTTP transfer: owner-only strong cell, Weak nonsecret seal.
//! No public response/body/token projection or arbitrary host callback.
use super::*;
use hyper::header::{HeaderMap, HeaderValue, AUTHORIZATION, COOKIE};
use std::sync::Weak;

pub(crate) type HttpOwner = RequestOwner;
pub(crate) struct HttpSeal {
    id: u64,
    cell: Weak<Mutex<Cell>>,
}
#[cfg(test)]
pub(crate) struct HttpWeak {
    cell: Weak<Mutex<Cell>>,
}
#[cfg(test)]
impl HttpWeak {
    pub(crate) fn has_live_owner(&self) -> bool {
        self.cell.strong_count() != 0
    }
}
// Only the private finalizer receives these transport-owned parts.
pub(crate) struct Wire {
    pub(crate) cookie: HeaderValue,
    pub(crate) body: hyper::body::Bytes,
}
impl RequestOwner {
    #[cfg(test)]
    pub(crate) fn has_wire_material_for_test(&self) -> bool {
        self.cell.lock().unwrap().pair.is_some()
    }
    #[cfg(test)]
    pub(crate) fn downgrade(&self) -> HttpWeak {
        HttpWeak {
            cell: Arc::downgrade(&self.cell),
        }
    }
    pub(crate) fn constrain_until(&self, until: Instant) -> Result<(), Failure> {
        let mut gate = self
            .lease
            .lease
            .checked_gate()
            .map_err(Failure::authority)?;
        gate.expires_at = gate.expires_at.min(until);
        if gate.expires_at <= Instant::now() {
            return Err(Failure::expired());
        }
        Ok(())
    }
    pub(crate) fn seal(&self) -> Result<HttpSeal, Failure> {
        let _gate = self
            .lease
            .lease
            .checked_gate()
            .map_err(Failure::authority)?;
        let mut c = self.cell.lock().map_err(|_| Failure::wire_internal())?;
        if !matches!(c.phase, Phase::Ready) {
            return Err(Failure::wire_internal());
        }
        if c.deadline.ok_or_else(Failure::wire_internal)? <= Instant::now() {
            return Err(Failure::expired());
        }
        c.phase = Phase::Sealed;
        Ok(HttpSeal {
            id: self.lease.lease.id,
            cell: Arc::downgrade(&self.cell),
        })
    }
    pub(crate) fn retire(&self) {
        let _gate = match self.lease.lease.gate.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        self.clear();
    }
}
impl Failure {
    pub(super) fn wire_internal() -> Self {
        Self {
            kind: FailureKind::Internal,
            ..Self::unavailable()
        }
    }
    pub(crate) fn http_status(&self) -> crate::http_server::Status {
        use crate::http_server::Status;
        match self.kind {
            FailureKind::InvalidRequest => Status::BAD_REQUEST,
            FailureKind::Denied | FailureKind::InvalidCredential => Status::FORBIDDEN,
            FailureKind::Expired => Status::GATEWAY_TIMEOUT,
            FailureKind::Unavailable => Status::SERVICE_UNAVAILABLE,
            FailureKind::Internal => Status::INTERNAL_SERVER_ERROR,
        }
    }
}
impl Pool {
    pub(crate) fn bound_to(&self, origin: &crate::http_server::BoundOrigin) -> bool {
        origin.matches(&self.origin)
    }
    pub(crate) async fn bootstrap_http(
        &self,
        owner: &HttpOwner,
        old: Option<Previous>,
    ) -> Result<(), Failure> {
        self.bootstrap(owner, old).await
    }
    pub(crate) fn previous_cookie(&self, headers: &HeaderMap) -> Result<Option<Previous>, Failure> {
        if headers.contains_key(AUTHORIZATION) {
            return Err(Failure::invalid());
        }
        let mut total = 0usize;
        let mut count = 0usize;
        let mut pairs = 0usize;
        let mut selected = None;
        for header in headers.get_all(COOKIE).iter() {
            count += 1;
            total = total
                .checked_add(header.as_bytes().len())
                .ok_or_else(Failure::invalid)?;
            if count > 64 || total > 8192 {
                return Err(Failure::invalid());
            }
            let text = header.to_str().map_err(|_| Failure::invalid())?;
            for pair in cookie::Cookie::split_parse(text) {
                pairs += 1;
                if pairs > 64 {
                    return Err(Failure::invalid());
                }
                let pair = pair.map_err(|_| Failure::invalid())?;
                if pair.name() == self.cookie.name {
                    if selected.is_some() {
                        return Err(Failure::invalid());
                    }
                    // This only names an old anonymous row for invalidation,
                    // never a claim proof or an identity/permission authority.
                    selected = Some((|| {
                        let value = pair.value();
                        if value.len() != 43 {
                            return Err(Failure::denied());
                        }
                        let mut decoded = [0u8; 33];
                        let n = URL_SAFE_NO_PAD
                            .decode_slice(value, &mut decoded)
                            .map_err(|_| Failure::denied())?;
                        if n != 32 || URL_SAFE_NO_PAD.encode(&decoded[..n]) != value {
                            return Err(Failure::denied());
                        }
                        Ok(Previous {
                            handle: Sha256::digest(&decoded[..n]).into(),
                        })
                    })());
                }
            }
        }
        selected.transpose()
    }
    pub(crate) fn take_wire(&self, owner: &HttpOwner, seal: HttpSeal) -> Result<Wire, Failure> {
        // Structural seal mismatch stays an invariant failure, independent of
        // a later clock/gate failure on the supplied foreign owner.
        if seal.id != owner.lease.lease.id
            || !Weak::ptr_eq(&seal.cell, &Arc::downgrade(&owner.cell))
        {
            return Err(Failure::wire_internal());
        }
        let clock = self.context.sample().map_err(Failure::session)?;
        let (pair, deadline, expiry, mut wall) = {
            let _gate = owner
                .lease
                .lease
                .checked_gate()
                .map_err(Failure::authority)?;
            let mut c = owner.cell.lock().map_err(|_| Failure::wire_internal())?;
            if !matches!(c.phase, Phase::Sealed) || c.pool != Some(self.incarnation) {
                return Err(Failure::wire_internal());
            }
            let deadline = c.deadline.ok_or_else(Failure::wire_internal)?;
            let expiry = c.expiry.ok_or_else(Failure::wire_internal)?;
            let clock = session::PreloginContext::track_sample(&mut c.wall, clock, 0)
                .map_err(Failure::session)?;
            if deadline <= Instant::now() || clock.ceil_ms >= expiry {
                return Err(Failure::expired());
            }
            let pair = c.pair.take().ok_or_else(Failure::wire_internal)?;
            let wall = c.wall;
            c.phase = Phase::Terminal;
            c.deadline = None;
            c.expiry = None;
            c.wall = None;
            c.pool = None;
            (pair, deadline, expiry, wall)
        };
        let same_site = match self.cookie.same_site {
            session::SameSite::Strict => cookie::SameSite::Strict,
            session::SameSite::Lax => cookie::SameSite::Lax,
            session::SameSite::None => return Err(Failure::wire_internal()),
        };
        let value = cookie::Cookie::build((self.cookie.name.clone(), pair.handle))
            .same_site(same_site)
            .secure(true)
            .http_only(true)
            .path("/")
            .build()
            .to_string();
        if value.len() > 512 {
            return Err(Failure::wire_internal());
        }
        let mut cookie = HeaderValue::from_str(&value).map_err(|_| Failure::wire_internal())?;
        cookie.set_sensitive(true);
        #[derive(serde::Serialize)]
        struct Json<'a> {
            csrf_token: &'a str,
        }
        let body = serde_json::to_vec(&Json {
            csrf_token: &pair.token,
        })
        .map_err(|_| Failure::wire_internal())?;
        if body.len() != 60 {
            return Err(Failure::wire_internal());
        }
        // Finite library serialization is not forcibly preemptible. Recheck
        // fresh observed UTC and live monotonic gates before wire ownership.
        let clock = self.context.sample().map_err(Failure::session)?;
        let _gate = owner
            .lease
            .lease
            .checked_gate()
            .map_err(Failure::authority)?;
        let clock = session::PreloginContext::track_sample(&mut wall, clock, 0)
            .map_err(Failure::session)?;
        if deadline <= Instant::now() || clock.ceil_ms >= expiry {
            return Err(Failure::expired());
        }
        Ok(Wire {
            cookie,
            body: hyper::body::Bytes::from(body),
        })
    }
    #[cfg(test)]
    pub(crate) fn wire_clock_for_test(
        &mut self,
        owner: &HttpOwner,
        samples: impl FnOnce(i64, i64) -> Vec<i64>,
    ) {
        // Numeric bounds only. No secret projection, no callback under cell lock.
        let (wall, expiry) = {
            let c = owner.cell.lock().unwrap();
            (c.wall.unwrap(), c.expiry.unwrap())
        };
        self.context.set_test_clock_sequence(samples(wall, expiry));
    }
    #[cfg(test)]
    pub(crate) async fn row_count_for_test(&self) -> i64 {
        let tx = sqlite::begin(&self.native, sqlite::BeginMode::Immediate)
            .await
            .unwrap();
        let count = sqlite::query::<Count>(
            &tx,
            sqlite::literal("SELECT count(*) AS n FROM __nagi_prelogin_ready"),
            sqlite::parameters(),
        )
        .await
        .unwrap()
        .unwrap()
        .0;
        sqlite::rollback(tx).await.unwrap();
        count
    }
    #[cfg(test)]
    pub(crate) fn entropy_calls_for_test(&self) -> usize {
        self.entropy_calls.load(std::sync::atomic::Ordering::SeqCst)
    }
    #[cfg(test)]
    pub(crate) fn business_calls_for_test(&self) -> usize {
        self.business_enqueues
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

// Closed extraction from checked HTTP options; no public origin-string factory.
pub(crate) async fn open_for_http(
    store: &session::Store,
    sql_options: sqlite::Options,
    options: Options,
    cookie: CookieOptions,
    host: &crate::http_server::Options,
) -> Result<Pool, Failure> {
    let origin =
        crate::http_server::checked_prelogin_origin(host).map_err(|_| Failure::invalid())?;
    super::open_bound(store, sql_options, options, cookie, &origin).await
}
