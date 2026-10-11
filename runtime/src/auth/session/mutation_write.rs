//! Shared fixed SQLite issue/rotation recipes. Request proofs are consumed only
//! at the real AuthScope entrypoints; trusted private primitives create no proof.
use super::{
    delivery, exec, metadata, secrets, Failure, Foundation, Intent, NativePlan, Progress, Reply,
    Snapshot,
};
use crate::{auth, sqlite};
use std::{sync::Arc, time::Instant};
pub(in crate::auth::session) enum EntropySource {
    Os,
    #[cfg(test)]
    Finite(std::collections::VecDeque<Result<secrets::SessionId, secrets::FailureKind>>),
}
impl EntropySource {
    fn next(&mut self) -> Result<secrets::SessionId, Failure> {
        match self {
            Self::Os => secrets::SessionId::generate().map_err(Failure::secret),
            #[cfg(test)]
            Self::Finite(values) => values
                .pop_front()
                .ok_or_else(Failure::unavailable)?
                .map_err(Failure::secret),
        }
    }
}
enum Operation {
    Issue { subject: i64, original: Instant },
    Rotate { old: Snapshot },
}
impl Operation {
    fn original(&self) -> Instant {
        match self {
            Self::Issue { original, .. } => *original,
            Self::Rotate { old } => old.absolute_expires_at,
        }
    }
    fn request_expired(&self) -> bool {
        self.original() <= Instant::now()
            || matches!(self,Self::Rotate{old} if old.expires_at <= Instant::now())
    }
}
pub(in crate::auth::session) struct PreparedWrite {
    pub(in crate::auth::session) tx: sqlite::Tx,
    pub(in crate::auth::session) context: WriteContext,
    pub(in crate::auth::session) progress: Progress,
}
pub(in crate::auth::session) struct WriteContext {
    lease: Arc<auth::Lease>,
    operation: Operation,
    digest: [u8; 32],
    pending: Option<delivery::Pending>,
    source: EntropySource,
    attempts: i64,
    #[cfg(test)]
    pause: Option<(
        tokio::sync::oneshot::Sender<()>,
        tokio::sync::oneshot::Receiver<()>,
    )>,
}
impl WriteContext {
    #[cfg(test)]
    pub(in crate::auth::session) fn pause_before_reserve(
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
fn policy(store: &Foundation) -> Result<secrets::CookiePolicy, Failure> {
    let same_site = match store.same_site {
        1 => cookie::SameSite::Strict,
        2 => cookie::SameSite::Lax,
        _ => return Err(Failure::invalid()),
    };
    secrets::CookiePolicy::new(&store.cookie_name, same_site).map_err(Failure::secret)
}
fn material(
    store: &Foundation,
    source: &mut EntropySource,
) -> Result<([u8; 32], secrets::SetCookie), Failure> {
    let id = source.next()?;
    let digest = id.digest().into_bytes();
    let cookie = policy(store)?.issue(id).map_err(Failure::secret)?;
    Ok((digest, cookie))
}
impl Foundation {
    pub(in crate::auth::session) async fn prepare_issue(
        &self,
        scope: auth::AuthScope,
    ) -> Result<PreparedWrite, Failure> {
        self.prepare_issue_with(scope, EntropySource::Os).await
    }
    pub(in crate::auth::session) async fn prepare_issue_with(
        &self,
        scope: auth::AuthScope,
        source: EntropySource,
    ) -> Result<PreparedWrite, Failure> {
        let auth::AuthScope {
            subject,
            lease,
            credential,
        } = scope;
        if !matches!(credential.source, auth::CredentialSource::Bearer) {
            return Err(Failure::denied());
        }
        self.prepare_write(
            lease,
            Operation::Issue {
                subject,
                original: credential.original_expires_at,
            },
            source,
        )
        .await
    }
    pub(in crate::auth::session) async fn prepare_rotate(
        &self,
        scope: auth::AuthScope,
    ) -> Result<PreparedWrite, Failure> {
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
            || old.absolute_expires_at != credential.original_expires_at
        {
            return Err(Failure::denied());
        }
        self.prepare_write(lease, Operation::Rotate { old }, EntropySource::Os)
            .await
    }
    async fn prepare_write(
        &self,
        lease: Arc<auth::Lease>,
        operation: Operation,
        mut source: EntropySource,
    ) -> Result<PreparedWrite, Failure> {
        if operation.request_expired() {
            return Err(super::expired());
        }
        let (digest, cookie) = material(self, &mut source)?;
        let pending = {
            let access = delivery::LeaseDelivery::for_lease(&lease).map_err(super::lease_error)?;
            access.stage(cookie).map_err(super::delivery_error)?
        };
        let (tx, clock, meta) = self.transaction().await?;
        Ok(PreparedWrite {
            tx,
            context: WriteContext {
                lease,
                operation,
                digest,
                pending: Some(pending),
                source,
                attempts: 1,
                #[cfg(test)]
                pause: None,
            },
            progress: Progress {
                wall: clock.ceil_ms.max(meta.last_wall),
                sent: false,
            },
        })
    }
    pub(in crate::auth::session) async fn issue_delivery(
        &self,
        scope: auth::AuthScope,
    ) -> Result<Intent, Failure> {
        let PreparedWrite {
            tx,
            context,
            mut progress,
        } = self.prepare_issue(scope).await?;
        let result = enqueue_write(self, &tx, context, &mut progress).await;
        complete_write(self, tx, result, progress).await
    }
    pub(in crate::auth::session) async fn rotate_delivery(
        &self,
        scope: auth::AuthScope,
    ) -> Result<Intent, Failure> {
        let PreparedWrite {
            tx,
            context,
            mut progress,
        } = self.prepare_rotate(scope).await?;
        let result = enqueue_write(self, &tx, context, &mut progress).await;
        complete_write(self, tx, result, progress).await
    }
}
struct CheckedRows {
    meta: super::super::Metadata,
}
async fn read_rows(
    store: &Foundation,
    tx: &sqlite::Tx,
    operation: &Operation,
    progress: &mut Progress,
) -> Result<CheckedRows, Failure> {
    let meta = metadata(tx).await?;
    if meta.version != 1
        || meta.incarnation.as_slice() != store.incarnation
        || meta.options != store.options
        || meta.cookie_name != store.cookie_name
        || meta.same_site != store.same_site
        || meta.options.max_row_bytes < super::super::ROW_BYTES
    {
        return Err(Failure::unavailable());
    }
    progress.wall = progress.wall.max(meta.last_wall);
    match operation {
        Operation::Issue { .. } => {
            let stored = sqlite::query::<super::super::Count>(
                tx,
                sqlite::literal("SELECT count(*) AS n FROM __nagi_session_rows"),
                sqlite::parameters(),
            )
            .await
            .map_err(Failure::database)?
            .ok_or_else(Failure::unavailable)?
            .0;
            let live=sqlite::query::<super::super::Count>(tx,sqlite::literal("SELECT count(*) AS n FROM __nagi_session_rows WHERE idle_ms>? AND absolute_ms>?"),sqlite::bind_i64(sqlite::bind_i64(sqlite::parameters(),progress.wall),progress.wall)).await.map_err(Failure::database)?.ok_or_else(Failure::unavailable)?.0;
            if stored >= store.options.max_stored || live >= store.options.max_live {
                return Err(Failure::unavailable());
            }
        }
        Operation::Rotate { old } => {
            let current=sqlite::query::<super::super::SessionRow>(tx,sqlite::literal("SELECT lineage,subject,digest,generation,idle_ms,absolute_ms FROM __nagi_session_rows WHERE lineage=?"),sqlite::bind_i64(sqlite::parameters(),old.lineage)).await.map_err(Failure::database)?.ok_or_else(Failure::denied)?;
            if old.incarnation != store.incarnation
                || current.subject != old.subject
                || current.digest.as_slice() != old.digest
                || current.generation != old.generation
                || current.absolute_ms != old.absolute_ms
                || current.idle_ms > current.absolute_ms
            {
                return Err(Failure::denied());
            }
        }
    }
    Ok(CheckedRows { meta })
}
async fn collision(tx: &sqlite::Tx, digest: [u8; 32]) -> Result<bool, Failure> {
    Ok(sqlite::query::<super::super::Count>(
        tx,
        sqlite::literal("SELECT count(*) AS n FROM __nagi_session_rows WHERE digest=?"),
        sqlite::bind_bytes(sqlite::parameters(), digest.to_vec()),
    )
    .await
    .map_err(Failure::database)?
    .ok_or_else(Failure::unavailable)?
    .0 != 0)
}
// Closed statement variants; no user SQL or callback can reach native admission.
pub(in crate::auth::session) enum Statement {
    Issue {
        lineage: i64,
        subject: i64,
        digest: [u8; 32],
        idle: i64,
        absolute: i64,
        wall: i64,
        incarnation: [u8; 32],
        max_stored: i64,
        max_live: i64,
    },
    Rotate {
        lineage: i64,
        subject: i64,
        digest: [u8; 32],
        old_digest: [u8; 32],
        old_generation: i64,
        generation: i64,
        idle: i64,
        absolute: i64,
        wall: i64,
        incarnation: [u8; 32],
    },
}
impl Statement {
    pub(in crate::auth::session) fn into_native(self) -> (sqlite::Query, sqlite::Parameters) {
        let mut p = sqlite::parameters();
        match self {
            Self::Issue {
                lineage,
                subject,
                digest,
                idle,
                absolute,
                wall,
                incarnation,
                max_stored,
                max_live,
            } => {
                for n in [lineage, subject] {
                    p = sqlite::bind_i64(p, n);
                }
                p = sqlite::bind_bytes(p, digest.to_vec());
                for n in [idle, absolute, lineage] {
                    p = sqlite::bind_i64(p, n);
                }
                p = sqlite::bind_bytes(p, incarnation.to_vec());
                for n in [wall, max_stored, wall, wall, max_live] {
                    p = sqlite::bind_i64(p, n);
                }
                (sqlite::literal("INSERT INTO __nagi_session_rows SELECT ?,?,?,0,?,? WHERE EXISTS(SELECT 1 FROM __nagi_session_meta WHERE id=1 AND version=1 AND next_lineage=? AND incarnation=? AND last_wall<=?) AND (SELECT count(*) FROM __nagi_session_rows)<? AND (SELECT count(*) FROM __nagi_session_rows WHERE idle_ms>? AND absolute_ms>?)<?"),p)
            }
            Self::Rotate {
                lineage,
                subject,
                digest,
                old_digest,
                old_generation,
                generation,
                idle,
                absolute,
                wall,
                incarnation,
            } => {
                p = sqlite::bind_bytes(p, digest.to_vec());
                for n in [generation, idle, lineage, subject] {
                    p = sqlite::bind_i64(p, n);
                }
                p = sqlite::bind_bytes(p, old_digest.to_vec());
                for n in [old_generation, absolute, wall, wall] {
                    p = sqlite::bind_i64(p, n);
                }
                p = sqlite::bind_bytes(p, incarnation.to_vec());
                p = sqlite::bind_i64(p, wall);
                (sqlite::literal("UPDATE __nagi_session_rows SET digest=?,generation=?,idle_ms=? WHERE lineage=? AND subject=? AND digest=? AND generation=? AND absolute_ms=? AND idle_ms>? AND absolute_ms>? AND EXISTS(SELECT 1 FROM __nagi_session_meta WHERE id=1 AND version=1 AND incarnation=? AND last_wall<=?)"),p)
            }
        }
    }
}
struct Completion {
    row: super::super::SessionRow,
    sample: super::super::ClockSample,
    next_lineage: Option<i64>,
}
fn fresh_plan(
    store: &Foundation,
    operation: &Operation,
    digest: [u8; 32],
    rows: CheckedRows,
    progress: &mut Progress,
) -> Result<(Statement, Completion), Failure> {
    let c = (store.clock)()?;
    Foundation::validate_clock(&c)?;
    if c.ceil_ms < progress.wall {
        return Err(Failure::unavailable());
    }
    progress.wall = c.ceil_ms;
    match operation {
        Operation::Issue { subject, original } => {
            let remaining = original
                .checked_duration_since(c.monotonic_after)
                .ok_or_else(Failure::credential)?;
            let ceiling = c
                .floor_ms
                .checked_add(
                    i64::try_from(remaining.as_millis()).map_err(|_| Failure::unavailable())?,
                )
                .ok_or_else(Failure::unavailable)?;
            let absolute = c
                .floor_ms
                .checked_add(store.options.absolute_ms)
                .ok_or_else(Failure::unavailable)?
                .min(ceiling);
            let idle = c
                .floor_ms
                .checked_add(store.options.idle_ms)
                .ok_or_else(Failure::unavailable)?
                .min(absolute);
            if idle <= c.ceil_ms {
                return Err(Failure::credential());
            }
            let lineage = rows.meta.next_lineage;
            let next = lineage.checked_add(1).ok_or_else(Failure::unavailable)?;
            let statement = Statement::Issue {
                lineage,
                subject: *subject,
                digest,
                idle,
                absolute,
                wall: c.ceil_ms,
                incarnation: store.incarnation,
                max_stored: store.options.max_stored,
                max_live: store.options.max_live,
            };
            Ok((
                statement,
                Completion {
                    row: super::super::SessionRow {
                        lineage,
                        subject: *subject,
                        digest: digest.to_vec(),
                        generation: 0,
                        idle_ms: idle,
                        absolute_ms: absolute,
                    },
                    sample: c,
                    next_lineage: Some(next),
                },
            ))
        }
        Operation::Rotate { old } => {
            if old.incarnation != store.incarnation
                || old.expires_at <= Instant::now()
                || old.absolute_expires_at <= Instant::now()
                || c.ceil_ms >= old.absolute_ms
            {
                return Err(Failure::denied());
            }
            let generation = old
                .generation
                .checked_add(1)
                .ok_or_else(Failure::unavailable)?;
            let idle = c
                .floor_ms
                .checked_add(store.options.idle_ms)
                .ok_or_else(Failure::unavailable)?
                .min(old.absolute_ms);
            if idle <= c.ceil_ms {
                return Err(Failure::denied());
            }
            let statement = Statement::Rotate {
                lineage: old.lineage,
                subject: old.subject,
                digest,
                old_digest: old.digest,
                old_generation: old.generation,
                generation,
                idle,
                absolute: old.absolute_ms,
                wall: c.ceil_ms,
                incarnation: store.incarnation,
            };
            Ok((
                statement,
                Completion {
                    row: super::super::SessionRow {
                        lineage: old.lineage,
                        subject: old.subject,
                        digest: digest.to_vec(),
                        generation,
                        idle_ms: idle,
                        absolute_ms: old.absolute_ms,
                    },
                    sample: c,
                    next_lineage: None,
                },
            ))
        }
    }
}
pub(in crate::auth::session) struct SubmittedWrite {
    lease: Arc<auth::Lease>,
    admitted: delivery::Admitted,
    reply: Reply,
    completion: Completion,
}
pub(in crate::auth::session) async fn enqueue_write(
    store: &Foundation,
    tx: &sqlite::Tx,
    mut context: WriteContext,
    progress: &mut Progress,
) -> Result<SubmittedWrite, Failure> {
    let rows = read_rows(store, tx, &context.operation, progress).await?;
    while collision(tx, context.digest).await? {
        if context.attempts >= store.options.collision_attempts {
            return Err(Failure::unavailable());
        }
        context.attempts += 1;
        // Still pre-admission: retire old Pending outside locks, then stage new
        // actual OS material before any next await. Never restage after a send.
        context.pending.take();
        let (digest, cookie) = material(store, &mut context.source)?;
        let pending = {
            let access =
                delivery::LeaseDelivery::for_lease(&context.lease).map_err(super::lease_error)?;
            access.stage(cookie).map_err(super::delivery_error)?
        };
        context.digest = digest;
        context.pending = Some(pending);
    }
    #[cfg(test)]
    if let Some((reached, resume)) = context.pause.take() {
        let _ = reached.send(());
        resume.await.map_err(|_| Failure::unavailable())?;
    }
    let reservation = tx.reserve_exec().await.map_err(Failure::database)?;
    let (statement, completion) =
        fresh_plan(store, &context.operation, context.digest, rows, progress)?;
    // This request-only immutable lookup limit is not the trusted primitive's
    // touched current-row lifetime. It remains checked AFTER actual reserve and
    // the fresh clock, before any gated send. Do not refresh it from the DB row.
    if context.operation.request_expired()
        || matches!(&context.operation, Operation::Rotate { old }
            if completion.sample.ceil_ms >= old.idle_ms
                || completion.sample.ceil_ms >= old.absolute_ms)
    {
        return Err(super::expired());
    }
    let (admitted, reply) = {
        let access =
            delivery::LeaseDelivery::for_lease(&context.lease).map_err(super::lease_error)?;
        access
            .enqueue_mutation(
                context.pending.take().ok_or_else(Failure::unavailable)?,
                reservation,
                NativePlan::Write(statement),
                &mut progress.sent,
            )
            .map_err(super::delivery_error)?
    };
    Ok(SubmittedWrite {
        lease: context.lease,
        admitted,
        reply,
        completion,
    })
}
async fn finish_written(
    store: &Foundation,
    tx: &sqlite::Tx,
    reply: Reply,
    completion: Completion,
) -> Result<Snapshot, Failure> {
    if reply.await.map_err(Failure::database)? != 1 {
        return Err(Failure::denied());
    }
    if let Some(next) = completion.next_lineage {
        let mut p = sqlite::bind_i64(sqlite::parameters(), next);
        p = sqlite::bind_i64(p, completion.row.lineage);
        p = sqlite::bind_bytes(p, store.incarnation.to_vec());
        if exec(tx,"UPDATE __nagi_session_meta SET next_lineage=? WHERE id=1 AND version=1 AND next_lineage=? AND incarnation=?",p).await?!=1 {return Err(Failure::unavailable());}
    }
    // Keep snapshot validation after business write; failure must roll back both
    // row and lineage counter (the original IR01 rule), without weak fallback.
    store.snapshot(completion.row, &completion.sample)
}
pub(in crate::auth::session) async fn complete_write(
    store: &Foundation,
    tx: sqlite::Tx,
    submitted: Result<SubmittedWrite, Failure>,
    progress: Progress,
) -> Result<Intent, Failure> {
    let result = match submitted {
        Ok(SubmittedWrite {
            lease,
            admitted,
            reply,
            completion,
        }) => match finish_written(store, &tx, reply, completion).await {
            Ok(_) => Ok((lease, admitted)),
            Err(e) => Err(e),
        },
        Err(e) => Err(e),
    };
    super::finish_ready(store, tx, result, progress).await
}

#[cfg(test)]
#[path = "mutation_fixtures.rs"]
pub(in crate::auth::session) mod fixtures;
