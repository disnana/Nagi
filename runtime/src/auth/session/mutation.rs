//! Private Session mutations: checked Pending -> actual enqueue -> full commit.
//! No HTTP finalization or public mutation API lives here.
use super::{delivery, exec, finish, metadata, secrets, Failure, Foundation, Snapshot};
use crate::{auth, sqlite};
use std::{future::Future, pin::Pin, sync::Arc, time::Instant};
#[path = "mutation_write.rs"]
pub(super) mod write;
// Only these closed, privately constructed statement recipes reach the sender.
pub(super) enum NativePlan {
    Logout(LogoutPlan),
    Write(write::Statement),
}
impl NativePlan {
    pub(super) fn into_native(self) -> (sqlite::Query, sqlite::Parameters) {
        match self {
            Self::Logout(plan) => plan.into_native(),
            Self::Write(plan) => plan.into_native(),
        }
    }
}
pub(super) type Reply = Pin<Box<dyn Future<Output = Result<i64, sqlite::Failure>> + Send>>;
pub(super) struct PreparedLogout {
    pub(super) tx: sqlite::Tx,
    pub(super) context: Context,
    pub(super) progress: Progress,
}
pub(super) struct Context {
    lease: Arc<auth::Lease>,
    old: Snapshot,
    original: Instant,
    pending: Option<delivery::Pending>,
    #[cfg(test)]
    pause: Option<(
        tokio::sync::oneshot::Sender<()>,
        tokio::sync::oneshot::Receiver<()>,
    )>,
}
impl Context {
    #[cfg(test)]
    pub(super) fn replace_pending_for_test(&mut self, pending: delivery::Pending) {
        self.pending = Some(pending);
    }
    #[cfg(test)]
    pub(super) fn pause_before_reserve(
        &mut self,
    ) -> (
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<()>,
    ) {
        let (reached, receive) = tokio::sync::oneshot::channel();
        let (release, resume) = tokio::sync::oneshot::channel();
        self.pause = Some((reached, resume));
        (receive, release)
    }
}
pub(super) struct Progress {
    wall: i64,
    sent: bool,
}
pub(super) struct Submitted {
    lease: Arc<auth::Lease>,
    pub(super) admitted: delivery::Admitted,
    pub(super) reply: Reply,
    pub(super) expected: i64,
}
// One-use opaque intent: origin retains only the Lease's Weak cell reference.
// A later request finalizer must check that origin; no material getter exists.
pub(super) struct Intent {
    _origin: Arc<auth::Lease>,
    _right: delivery::ReadyClaim,
}
impl Intent {
    pub(in crate::auth::session) fn apply(self) -> Result<delivery::Seal, Failure> {
        let Self { _origin, _right } = self;
        let access = delivery::LeaseDelivery::for_lease(&_origin).map_err(lease_error)?;
        access.apply_ready(_right).map_err(|error| match error {
            delivery::Error::Unavailable => Failure::unavailable(),
            _ => Failure::kind(auth::FailureKind::Internal),
        })
    }
}
fn lease_error(error: auth::Failure) -> Failure {
    Failure::kind(error.kind)
}
fn delivery_error(_: delivery::Error) -> Failure {
    Failure::unavailable()
}
fn expired() -> Failure {
    Failure::kind(auth::FailureKind::Expired)
}
// Closed statement plan; no host-provided SQL, callbacks, or authority booleans.
pub(super) struct LogoutPlan {
    lineage: i64,
    subject: i64,
    generation: i64,
    wall: i64,
    incarnation: [u8; 32],
}
impl LogoutPlan {
    pub(super) fn into_native(self) -> (sqlite::Query, sqlite::Parameters) {
        let mut p = sqlite::parameters();
        for value in [
            self.lineage,
            self.subject,
            self.generation,
            self.wall,
            self.wall,
        ] {
            p = sqlite::bind_i64(p, value);
        }
        p = sqlite::bind_bytes(p, self.incarnation.to_vec());
        p = sqlite::bind_i64(p, self.wall);
        (
            sqlite::literal(
                "DELETE FROM __nagi_session_rows WHERE lineage=? AND subject=? AND generation=? AND idle_ms>? AND absolute_ms>? AND EXISTS(SELECT 1 FROM __nagi_session_meta WHERE id=1 AND version=1 AND incarnation=? AND last_wall<=?)",
            ),
            p,
        )
    }
}
impl Foundation {
    pub(super) async fn prepare_logout(
        &self,
        scope: auth::AuthScope,
    ) -> Result<PreparedLogout, Failure> {
        let auth::AuthScope {
            subject,
            lease,
            credential,
        } = scope;
        let auth::CredentialSource::Session(old) = credential.source else {
            return Err(Failure::denied());
        };
        if old.subject != subject
            || old.incarnation != self.incarnation
            || credential.original_expires_at != old.absolute_expires_at
        {
            return Err(Failure::denied());
        }
        if old.expires_at <= Instant::now() || credential.original_expires_at <= Instant::now() {
            return Err(expired());
        }
        let same_site = match self.same_site {
            1 => cookie::SameSite::Strict,
            2 => cookie::SameSite::Lax,
            _ => return Err(Failure::invalid()), // None requires the future SF03 guard.
        };
        let material = secrets::CookiePolicy::new(&self.cookie_name, same_site)
            .map_err(Failure::secret)?
            .remove()
            .map_err(Failure::secret)?;
        let pending = {
            let access = delivery::LeaseDelivery::for_lease(&lease).map_err(lease_error)?;
            access.stage(material).map_err(delivery_error)?
        }; // Gate and temporary strong upgrade are gone before DB acquisition.
        let (tx, clock, meta) = self.transaction().await?;
        Ok(PreparedLogout {
            tx,
            context: Context {
                lease,
                old,
                original: credential.original_expires_at,
                pending: Some(pending),
                #[cfg(test)]
                pause: None,
            },
            progress: Progress {
                wall: clock.ceil_ms.max(meta.last_wall),
                sent: false,
            },
        })
    }
    pub(super) async fn logout_delivery(&self, scope: auth::AuthScope) -> Result<Intent, Failure> {
        let PreparedLogout {
            tx,
            context,
            mut progress,
        } = self.prepare_logout(scope).await?;
        let result = enqueue_logout(self, &tx, context, &mut progress).await;
        complete_logout(self, tx, result, progress).await
    }
}
pub(super) async fn enqueue_logout(
    store: &Foundation,
    tx: &sqlite::Tx,
    mut context: Context,
    progress: &mut Progress,
) -> Result<Submitted, Failure> {
    // Re-read after maintenance in this same privately held Immediate Tx. No
    // other production caller owns the Tx while reserve is pending. Native SQL
    // repeats incarnation/wall/current generation/expiry predicates at execution.
    let meta = metadata(tx).await?;
    if meta.version != 1
        || meta.incarnation.as_slice() != store.incarnation
        || meta.options != store.options
        || meta.cookie_name != store.cookie_name
        || meta.same_site != store.same_site
    {
        return Err(Failure::unavailable());
    }
    progress.wall = progress.wall.max(meta.last_wall);
    let row = sqlite::query::<super::SessionRow>(tx,
        sqlite::literal("SELECT lineage,subject,digest,generation,idle_ms,absolute_ms FROM __nagi_session_rows WHERE lineage=?"),
        sqlite::bind_i64(sqlite::parameters(), context.old.lineage)).await.map_err(Failure::database)?;
    if row.as_ref().is_some_and(|r| {
        r.subject != context.old.subject
            || r.generation < context.old.generation
            || r.digest.len() != 32
            || r.idle_ms > r.absolute_ms
    }) {
        return Err(Failure::denied());
    }
    #[cfg(test)]
    if let Some((reached, resume)) = context.pause.take() {
        let _ = reached.send(());
        resume.await.map_err(|_| Failure::unavailable())?;
    }
    let reservation = tx.reserve_exec().await.map_err(Failure::database)?;
    // Trusted clock is invoked outside either lock, after real capacity wait.
    let sample = (store.clock)()?;
    Foundation::validate_clock(&sample)?;
    if sample.ceil_ms < progress.wall {
        return Err(Failure::unavailable());
    }
    progress.wall = sample.ceil_ms;
    if sample.ceil_ms >= context.old.idle_ms
        || sample.ceil_ms >= context.old.absolute_ms
        || context.old.expires_at <= Instant::now()
        || context.original <= Instant::now()
        || row
            .as_ref()
            .is_some_and(|r| sample.ceil_ms >= r.idle_ms || sample.ceil_ms >= r.absolute_ms)
    {
        return Err(expired());
    }
    let expected = i64::from(row.is_some());
    let plan = LogoutPlan {
        lineage: context.old.lineage,
        subject: context.old.subject,
        generation: row.map_or(context.old.generation, |r| r.generation),
        wall: sample.ceil_ms,
        incarnation: store.incarnation,
    };
    let (admitted, reply) = {
        let access = delivery::LeaseDelivery::for_lease(&context.lease).map_err(lease_error)?;
        // The actual synchronous native send is inside the live gate. The slot
        // was staged before waiting; cell mutex is released before send/Drop.
        access
            .enqueue_logout(
                context.pending.take().ok_or_else(Failure::unavailable)?,
                reservation,
                plan,
                &mut progress.sent,
            )
            .map_err(delivery_error)?
    };
    Ok(Submitted {
        lease: context.lease,
        admitted,
        reply,
        expected,
    })
}
pub(super) async fn complete_logout(
    store: &Foundation,
    tx: sqlite::Tx,
    submitted: Result<Submitted, Failure>,
    progress: Progress,
) -> Result<Intent, Failure> {
    let result = match submitted {
        Ok(submitted) => match submitted.reply.await {
            Ok(affected) if affected == submitted.expected => {
                Ok((submitted.lease, submitted.admitted))
            }
            Ok(_) => Err(Failure::unavailable()),
            Err(error) => Err(Failure::database(error)),
        },
        Err(error) => Err(error),
    };
    finish_ready(store, tx, result, progress).await
}
// Common full-finish/one-use delivery boundary for logout, issue and rotation.
async fn finish_ready(
    store: &Foundation,
    tx: sqlite::Tx,
    result: Result<(Arc<auth::Lease>, delivery::Admitted), Failure>,
    progress: Progress,
) -> Result<Intent, Failure> {
    // Record only the validated high water; do not run a second cleanup batch.
    // Before admission the transaction still contains maintenance only. After
    // any real send, existing finish rolls failed business work back and performs
    // at most one bounded maintenance restoration transaction.
    let result = match exec(
        &tx,
        "UPDATE __nagi_session_meta SET last_wall=MAX(last_wall,?) WHERE id=1",
        sqlite::bind_i64(sqlite::parameters(), progress.wall),
    )
    .await
    {
        Ok(_) => result,
        Err(error) => Err(error),
    };
    let (lease, admitted) =
        finish(tx, result, progress.sent.then_some((store, progress.wall))).await?;
    let right = {
        let access = delivery::LeaseDelivery::for_lease(&lease).map_err(|error| {
            let mut error = lease_error(error);
            error.outcome = sqlite::Outcome::Committed;
            error
        })?;
        access.confirm_committed(admitted).map_err(|error| {
            let mut error = delivery_error(error);
            error.outcome = sqlite::Outcome::Committed;
            error
        })?
    };
    Ok(Intent {
        _origin: lease,
        _right: right,
    })
}
