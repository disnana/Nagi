//! Closed Session-to-HTTP identity bridge; no raw credential getter.
use super::Store;
use crate::auth::{Failure, VerifiedIdentity};
use hyper::header::{HeaderMap, AUTHORIZATION, COOKIE};
use std::{sync::Arc, time::Instant};

impl Store {
    pub(crate) async fn lookup_http_identity(
        &self,
        headers: &HeaderMap,
    ) -> Result<VerifiedIdentity, Failure> {
        // The existing CookiePolicy allows at most64 headers. Retain at most65
        // borrowed values: the overflow sentinel is rejected by that same parser
        // before ID decoding/SQL. No header bytes or secrets are copied here.
        if headers.contains_key(AUTHORIZATION) {
            return Err(Failure::invalid_request());
        }
        let values = headers
            .get_all(COOKIE)
            .iter()
            .take(65)
            .map(|value| {
                std::str::from_utf8(value.as_bytes()).map_err(|_| Failure::invalid_request())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let snapshot = self
            .foundation
            .lookup_cookie_mode(&values, false, false)
            .await
            .map_err(|failure| Failure { kind: failure.kind })?;
        let touch = HttpTouch::new(Arc::clone(&self.foundation), &snapshot);
        let mut identity = snapshot.into_verified_identity()?;
        identity.pending_http_touch = Some(touch);
        Ok(identity)
    }
}
// Terminal disposal is allowed even after expiry/poison. Always gate -> cell.
// There is no callback, await, raw owner getter or extra strong cell owner.
pub(crate) fn retire_http_delivery(owner: &crate::auth::LeaseOwner) {
    let _gate = match owner.lease.gate.lock() {
        Ok(gate) => gate,
        Err(error) => error.into_inner(),
    };
    owner.delivery.clear();
}
pub(crate) fn take_http_cookie(
    owner: &crate::auth::LeaseOwner,
    seal: super::ResponseSeal,
) -> Result<hyper::header::HeaderValue, ()> {
    let access = owner.checked_delivery(&owner.lease).map_err(|_| ())?;
    access
        .take_applied(seal.0)
        .map(|material| material.into_header())
        .map_err(|_| ())
}

#[cfg(test)]
#[path = "http_bridge_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "delivery_wire_tests.rs"]
mod delivery_tests;

#[cfg(test)]
#[path = "http_rate_tests.rs"]
mod rate_tests;

// Existing issuer bound, shared with HTTP preflight before destructive take.
pub(crate) fn http_cookie_maximum() -> usize {
    super::secrets::MAX_SET_COOKIE_BYTES
}

// Request-private deferred idle refresh. This is metadata, not a second proof
// or issuer. Only an actual read snapshot/Store can construct it; no Clone.
pub(in crate::auth) struct HttpTouch {
    foundation: Arc<super::Foundation>,
    lineage: i64,
    subject: i64,
    digest: [u8; 32],
    generation: i64,
    idle_ms: i64,
    absolute_ms: i64,
    expires_at: Instant,
    absolute_expires_at: Instant,
}
impl HttpTouch {
    fn new(foundation: Arc<super::Foundation>, snapshot: &super::Snapshot) -> Self {
        Self {
            foundation,
            lineage: snapshot.lineage,
            subject: snapshot.subject,
            digest: snapshot.digest,
            generation: snapshot.generation,
            idle_ms: snapshot.idle_ms,
            absolute_ms: snapshot.absolute_ms,
            expires_at: snapshot.expires_at,
            absolute_expires_at: snapshot.absolute_expires_at,
        }
    }
    fn live(&self, lease: &crate::auth::Lease) -> Result<(), super::Failure> {
        lease
            .validate()
            .map_err(|e| super::Failure::kind(e.kind()))?;
        if self.expires_at <= Instant::now() || self.absolute_expires_at <= Instant::now() {
            return Err(super::Failure::kind(crate::auth::FailureKind::Expired));
        }
        Ok(())
    }
    pub(in crate::auth) async fn apply(
        self,
        lease: Arc<crate::auth::Lease>,
    ) -> Result<(), crate::auth::Failure> {
        self.apply_inner(&lease)
            .await
            .map_err(|e| crate::auth::Failure { kind: e.kind })
    }
    async fn apply_inner(&self, lease: &crate::auth::Lease) -> Result<(), super::Failure> {
        self.live(lease)?;
        let store = &self.foundation;
        let (tx, c, meta) = store.transaction().await?;
        let mut wall = c.ceil_ms.max(meta.last_wall);
        let mut sent = false;
        let result=async {
            self.live(lease)?;
            let p=crate::sqlite::bind_i64(crate::sqlite::bind_bytes(crate::sqlite::bind_i64(crate::sqlite::parameters(),self.lineage),self.digest.to_vec()),self.generation);
            let row=crate::sqlite::query::<super::SessionRow>(&tx,crate::sqlite::literal("SELECT lineage,subject,digest,generation,idle_ms,absolute_ms FROM __nagi_session_rows WHERE lineage=? AND digest=? AND generation=?"),p).await.map_err(super::Failure::database)?.ok_or_else(super::Failure::credential)?;
            if row.subject!=self.subject || row.absolute_ms!=self.absolute_ms || row.idle_ms<self.idle_ms || row.idle_ms>row.absolute_ms || row.idle_ms<=c.ceil_ms || row.absolute_ms<=c.ceil_ms { return Err(super::Failure::denied()); }
            let last=row.idle_ms.checked_sub(store.options.idle_ms).ok_or_else(super::Failure::unavailable)?;
            if c.floor_ms.checked_sub(last).ok_or_else(super::Failure::unavailable)?<store.options.touch_ms { return Ok(()); }
            let reservation=tx.reserve_exec().await.map_err(super::Failure::database)?;
            // Sample and construct the bounded recipe outside the short gate.
            let sample=(store.clock)()?;
            super::Foundation::validate_clock(&sample)?;
            if sample.ceil_ms<wall { return Err(super::Failure::unavailable()); }
            wall=sample.ceil_ms;
            self.live(lease)?;
            if sample.ceil_ms>=row.idle_ms || sample.ceil_ms>=row.absolute_ms { return Err(super::Failure::kind(crate::auth::FailureKind::Expired)); }
            let next=sample.floor_ms.checked_add(store.options.idle_ms).ok_or_else(super::Failure::unavailable)?.min(row.absolute_ms);
            if next<=row.idle_ms { return Ok(()); }
            let mut p=crate::sqlite::bind_i64(crate::sqlite::parameters(),next);
            for n in [self.lineage,self.subject,self.generation,self.absolute_ms,row.idle_ms] { p=crate::sqlite::bind_i64(p,n); }
            p=crate::sqlite::bind_bytes(p,self.digest.to_vec());
            p=crate::sqlite::bind_i64(crate::sqlite::bind_i64(p,wall),wall);
            p=crate::sqlite::bind_bytes(p,store.incarnation.to_vec());
            p=crate::sqlite::bind_i64(p,wall);
            let query=crate::sqlite::literal("UPDATE __nagi_session_rows SET idle_ms=? WHERE lineage=? AND subject=? AND generation=? AND absolute_ms=? AND idle_ms=? AND digest=? AND idle_ms>? AND absolute_ms>? AND EXISTS(SELECT 1 FROM __nagi_session_meta WHERE id=1 AND version=1 AND incarnation=? AND last_wall<=?)");
            let reply={
                let _gate=lease.checked_gate().map_err(|e| super::Failure::kind(e.kind()))?;
                if self.expires_at<=Instant::now() || self.absolute_expires_at<=Instant::now() { return Err(super::Failure::kind(crate::auth::FailureKind::Expired)); }
                sent=true;
                reservation.enqueue(query,p)
            };
            if reply.await.map_err(super::Failure::database)?!=1 { return Err(super::Failure::denied()); }
            self.live(lease)?;
            Ok(())
        }.await;
        let result = match super::exec(
            &tx,
            "UPDATE __nagi_session_meta SET last_wall=MAX(last_wall,?) WHERE id=1",
            crate::sqlite::bind_i64(crate::sqlite::parameters(), wall),
        )
        .await
        {
            Ok(_) => result,
            Err(error) => Err(error),
        };
        // Failed attempted business touch rolls back before restoring only the
        // existing bounded maintenance observation. No retry/lease extension.
        super::finish(tx, result, sent.then_some((store.as_ref(), wall))).await
    }
}
