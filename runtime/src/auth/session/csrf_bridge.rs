//! Private immutable admission copy for the authenticated B1 checkpoint.
use super::*;

pub(crate) struct CsrfBinding {
    snapshot: Snapshot,
    request: u64,
}
impl CsrfBinding {
    pub(crate) fn from_identity(
        identity: &crate::auth::VerifiedIdentity,
        lease: &crate::auth::Lease,
    ) -> Result<Self, crate::auth::Failure> {
        identity.credential.validate()?;
        lease.validate()?;
        let crate::auth::CredentialSource::Session(s) = &identity.credential.source else {
            return Err(crate::auth::Failure::internal());
        };
        Ok(Self {
            snapshot: Snapshot {
                incarnation: s.incarnation,
                lineage: s.lineage,
                subject: s.subject,
                digest: s.digest,
                generation: s.generation,
                idle_ms: s.idle_ms,
                absolute_ms: s.absolute_ms,
                expires_at: s.expires_at,
                absolute_expires_at: s.absolute_expires_at,
            },
            request: lease.id,
        })
    }
    pub(crate) fn absolute_ms(&self) -> i64 {
        self.snapshot.absolute_ms
    }
    pub(crate) fn absolute_deadline(&self) -> Instant {
        self.snapshot.absolute_expires_at
    }
    pub(crate) fn token_parameters(&self, p: sqlite::Parameters) -> sqlite::Parameters {
        let p = sqlite::bind_bytes(p, self.snapshot.incarnation.to_vec());
        let p = sqlite::bind_i64(p, self.snapshot.lineage);
        sqlite::bind_i64(p, self.snapshot.generation)
    }
    // Exact current-row INSERT predicate. Only this module can inspect the copy.
    pub(crate) fn current_parameters(
        &self,
        p: sqlite::Parameters,
        wall: i64,
    ) -> sqlite::Parameters {
        let s = &self.snapshot;
        let p = sqlite::bind_i64(p, s.lineage);
        let p = sqlite::bind_i64(p, s.subject);
        let p = sqlite::bind_bytes(p, s.digest.to_vec());
        let p = sqlite::bind_i64(p, s.generation);
        let p = sqlite::bind_i64(p, s.absolute_ms);
        let p = sqlite::bind_i64(p, wall);
        sqlite::bind_i64(p, wall)
    }
    pub(in crate::auth) fn check_clock(
        &self,
        c: &PreloginClock,
        lease: &crate::auth::Lease,
    ) -> Result<(), Failure> {
        if self.request != lease.id {
            return Err(Failure::unavailable());
        }
        let s = &self.snapshot;
        if c.ceil_ms >= s.idle_ms
            || c.ceil_ms >= s.absolute_ms
            || Instant::now() >= s.expires_at
            || Instant::now() >= s.absolute_expires_at
        {
            return Err(Failure::denied());
        }
        Ok(())
    }
}
impl Store {
    pub(in crate::auth) fn csrf_context(&self) -> Result<(sqlite::Pool, PreloginContext), Failure> {
        let source = self
            .foundation
            .main_source
            .as_ref()
            .ok_or_else(Failure::unavailable)?
            .clone();
        Ok((
            sqlite::clone_pool(&self.foundation.pool),
            PreloginContext {
                foundation: Arc::clone(&self.foundation),
                source,
                #[cfg(test)]
                test_clock: None,
            },
        ))
    }
}
impl PreloginContext {
    pub(in crate::auth) async fn lookup_http_identity(
        &self,
        headers: &hyper::HeaderMap,
    ) -> Result<crate::auth::VerifiedIdentity, crate::auth::Failure> {
        Store {
            foundation: Arc::clone(&self.foundation),
        }
        .lookup_http_identity(headers)
        .await
    }
    pub(in crate::auth) fn matches_store(&self, store: &Store) -> bool {
        let f = &store.foundation;
        f.main_source.as_ref() == Some(&self.source)
            && f.incarnation == self.foundation.incarnation
            && f.options == self.foundation.options
            && f.cookie_name == self.foundation.cookie_name
            && f.same_site == self.foundation.same_site
    }
    pub(in crate::auth) async fn verify_binding(
        &self,
        tx: &sqlite::Tx,
        b: &CsrfBinding,
        c: &PreloginClock,
        lease: &crate::auth::Lease,
    ) -> Result<(), Failure> {
        b.check_clock(c, lease)?;
        if b.snapshot.incarnation != self.foundation.incarnation {
            return Err(Failure::unavailable());
        }
        let row = sqlite::query::<SessionRow>(tx,
            sqlite::literal("SELECT lineage,subject,digest,generation,idle_ms,absolute_ms FROM __nagi_session_rows WHERE lineage=?"),
            sqlite::bind_i64(sqlite::parameters(), b.snapshot.lineage)).await.map_err(Failure::database)?;
        let Some(r) = row else {
            return Err(Failure::denied());
        };
        let s = &b.snapshot;
        if r.subject != s.subject
            || r.digest.as_slice() != s.digest
            || r.generation != s.generation
            || r.absolute_ms != s.absolute_ms
            || r.idle_ms < s.idle_ms
            || r.idle_ms <= c.ceil_ms
            || r.absolute_ms <= c.ceil_ms
        {
            return Err(Failure::denied());
        }
        Ok(())
    }
}
