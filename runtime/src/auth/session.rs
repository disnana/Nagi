//! Checked persistent Session startup over the existing SQLite Pool.
//! Public checked startup and one-use request delivery use private admission bridges.
//! No raw issuer, digest, clock, Pool conversion, or generic protected SQL API.
use super::FailureKind;
use crate::{sqlite, FromRow};
use std::{
    fmt,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
#[path = "session/delivery.rs"]
pub(super) mod delivery;
#[path = "session/http_bridge.rs"]
mod http_bridge;
pub(crate) use http_bridge::{retire_http_delivery, take_http_cookie};
#[path = "session/mutation.rs"]
mod mutation;
#[path = "session/secrets.rs"]
mod secrets;
#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;

const ROW_BYTES: i64 = 72; // five i64 fields plus a 32-byte digest; not DB pages.
const META: &str = "CREATE TABLE __nagi_session_meta(id INTEGER PRIMARY KEY CHECK(id=1),version INTEGER NOT NULL,incarnation BLOB NOT NULL CHECK(length(incarnation)=32),last_wall INTEGER NOT NULL CHECK(last_wall>=0),next_lineage INTEGER NOT NULL CHECK(next_lineage>0),max_live INTEGER NOT NULL,max_stored INTEGER NOT NULL,max_row_bytes INTEGER NOT NULL,cleanup_batch INTEGER NOT NULL,idle_ms INTEGER NOT NULL,absolute_ms INTEGER NOT NULL,touch_ms INTEGER NOT NULL,collision_attempts INTEGER NOT NULL,cookie_name TEXT NOT NULL,same_site INTEGER NOT NULL) STRICT";
const ROWS: &str = "CREATE TABLE __nagi_session_rows(lineage INTEGER PRIMARY KEY CHECK(lineage>0),subject INTEGER NOT NULL,digest BLOB NOT NULL CHECK(length(digest)=32),generation INTEGER NOT NULL CHECK(generation>=0),idle_ms INTEGER NOT NULL,absolute_ms INTEGER NOT NULL CHECK(idle_ms<=absolute_ms)) STRICT";
const INDEX: &str = "CREATE UNIQUE INDEX __nagi_session_digest ON __nagi_session_rows(digest)";

pub struct Failure {
    kind: FailureKind,
    outcome: sqlite::Outcome,
}
impl Failure {
    fn kind(kind: FailureKind) -> Self {
        Self {
            kind,
            outcome: sqlite::Outcome::NotApplicable,
        }
    }
    fn invalid() -> Self {
        Self::kind(FailureKind::InvalidRequest)
    }
    fn unavailable() -> Self {
        Self::kind(FailureKind::Unavailable)
    }
    fn credential() -> Self {
        Self::kind(FailureKind::InvalidCredential)
    }
    fn denied() -> Self {
        Self::kind(FailureKind::Denied)
    }
    fn secret(error: secrets::FailureKind) -> Self {
        Self::kind(match error {
            secrets::FailureKind::InvalidRequest => FailureKind::InvalidRequest,
            secrets::FailureKind::InvalidCredential => FailureKind::InvalidCredential,
            secrets::FailureKind::Unavailable => FailureKind::Unavailable,
        })
    }
    fn database(error: sqlite::Failure) -> Self {
        Self {
            kind: FailureKind::Unavailable,
            outcome: error.outcome,
        }
    }
}
impl fmt::Debug for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionFailure")
            .field("kind", &self.kind)
            .field("outcome", &self.outcome)
            .finish()
    }
}
/// Stable secret-free classification, without the SQLite private cause.
pub fn kind(failure: &Failure) -> FailureKind {
    failure.kind
}
/// Stable message borrowed from the opaque failure.
pub fn message(failure: &Failure) -> &str {
    match failure.kind {
        FailureKind::InvalidCredential => "invalid credential",
        FailureKind::Denied => "permission denied",
        FailureKind::Expired => "authority expired",
        FailureKind::InvalidRequest => "invalid security request",
        FailureKind::Unavailable => "security service unavailable",
        FailureKind::Internal => "security service failure",
    }
}
/// Retain the native database outcome separately from authentication classification.
pub fn outcome(failure: &Failure) -> sqlite::Outcome {
    failure.outcome
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(message(self))
    }
}
impl std::error::Error for Failure {}
#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    max_live: i64,
    max_stored: i64,
    max_row_bytes: i64,
    cleanup_batch: i64,
    idle_ms: i64,
    absolute_ms: i64,
    touch_ms: i64,
    collision_attempts: i64,
}
pub fn options(
    max_live: i64,
    max_stored: i64,
    max_row_bytes: i64,
    cleanup_batch: i64,
    idle_ms: i64,
    absolute_ms: i64,
    touch_ms: i64,
    collision_attempts: i64,
) -> Result<Options, Failure> {
    if [
        max_live,
        max_stored,
        max_row_bytes,
        cleanup_batch,
        idle_ms,
        absolute_ms,
        touch_ms,
        collision_attempts,
    ]
    .iter()
    .any(|x| *x <= 0 || usize::try_from(*x).is_err())
        || max_live > max_stored
        || max_row_bytes < ROW_BYTES
        || cleanup_batch > max_stored
        || touch_ms > idle_ms
        || idle_ms > absolute_ms
    {
        return Err(Failure::invalid());
    }
    Ok(Options {
        max_live,
        max_stored,
        max_row_bytes,
        cleanup_batch,
        idle_ms,
        absolute_ms,
        touch_ms,
        collision_attempts,
    })
}
/// Browser SameSite configuration. None remains guarded by SF03 before use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SameSite {
    Strict,
    Lax,
    None,
}
/// Checked nonsecret cookie configuration with no weakening setters.
pub struct CookieOptions {
    policy: secrets::CookiePolicy,
}
impl fmt::Debug for CookieOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionCookieOptions")
            .finish_non_exhaustive()
    }
}
pub fn cookie_options(name: &str, same_site: SameSite) -> Result<CookieOptions, Failure> {
    let same_site = match same_site {
        SameSite::Strict => cookie::SameSite::Strict,
        SameSite::Lax => cookie::SameSite::Lax,
        SameSite::None => cookie::SameSite::None,
    };
    Ok(CookieOptions {
        policy: secrets::CookiePolicy::new(name, same_site).map_err(Failure::secret)?,
    })
}
/// One-use committed delivery right. No secret getter, Clone, Debug or Serde.
pub struct SessionResponse {
    intent: mutation::Intent,
}
// Stored in ordinary Response, containing only the nonsecret request seal.
pub(crate) struct ResponseSeal(delivery::Seal);

pub async fn issue(store: &Store, scope: super::AuthScope) -> Result<SessionResponse, Failure> {
    store
        .foundation
        .issue_delivery(scope)
        .await
        .map(|intent| SessionResponse { intent })
}
pub async fn rotate(store: &Store, scope: super::AuthScope) -> Result<SessionResponse, Failure> {
    store
        .foundation
        .rotate_delivery(scope)
        .await
        .map(|intent| SessionResponse { intent })
}
pub async fn logout(store: &Store, scope: super::AuthScope) -> Result<SessionResponse, Failure> {
    store
        .foundation
        .logout_delivery(scope)
        .await
        .map(|intent| SessionResponse { intent })
}
pub fn apply(
    response: crate::http_server::Response,
    intent: SessionResponse,
) -> Result<crate::http_server::Response, Failure> {
    if response.has_session_seal() {
        // Dropping the Ready claim retires its origin material; the ordinary
        // response retains no secret/proof/lease, only its nonsecret seal.
        return Err(Failure::kind(FailureKind::Internal));
    }
    let seal = intent.intent.apply()?;
    Ok(response.with_session_seal(ResponseSeal(seal)))
}

/// Immutable persistent Store state, never an authorization proof.
/// Only checked startup can construct it. No implicit Clone, raw parts, or close API.
pub struct Store {
    foundation: Arc<Foundation>,
}
impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionStore").finish_non_exhaustive()
    }
}
/// Trusted startup over a persistent Pool; configuration/schema checks and commit
/// precede success. Drop releases a handle; the original Pool owner observes close/join.
pub async fn open(
    pool: &sqlite::Pool,
    options: Options,
    cookie: CookieOptions,
) -> Result<Store, Failure> {
    let same_site = match cookie.policy.same_site_code() {
        1 => cookie::SameSite::Strict,
        2 => cookie::SameSite::Lax,
        _ => return Err(Failure::invalid()),
    };
    let foundation = Foundation::initialize_with_os_secrets(
        pool,
        options,
        cookie.policy.name(),
        same_site,
        Arc::new(wall_clock),
    )
    .await?;
    Ok(Store {
        foundation: Arc::new(foundation),
    })
}
/// Explicitly share immutable Store state; no snapshot/lease/authorization is cloned.
pub fn clone_store(store: &Store) -> Store {
    Store {
        foundation: Arc::clone(&store.foundation),
    }
}

pub(super) struct ClockSample {
    monotonic: Instant,
    monotonic_after: Instant,
    floor_ms: i64,
    ceil_ms: i64,
}
pub(super) type Clock = Arc<dyn Fn() -> Result<ClockSample, Failure> + Send + Sync>;
pub(super) fn wall_clock() -> Result<ClockSample, Failure> {
    let monotonic = Instant::now();
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Failure::unavailable())?;
    let monotonic_after = Instant::now();
    let floor_ms = i64::try_from(duration.as_millis()).map_err(|_| Failure::unavailable())?;
    let ceil_ms = if duration.subsec_nanos() % 1_000_000 != 0 {
        floor_ms.checked_add(1).ok_or_else(Failure::unavailable)?
    } else {
        floor_ms
    };
    Ok(ClockSample {
        monotonic,
        monotonic_after,
        floor_ms,
        ceil_ms,
    })
}
// No Debug/Clone/Serde or public digest/lineage parts. Only a private DB-layer
// copy supports concurrent oracle inputs; this is not an AuthScope/Grant copy.
pub(super) struct Snapshot {
    incarnation: [u8; 32],
    lineage: i64,
    subject: i64,
    digest: [u8; 32],
    generation: i64,
    idle_ms: i64,
    absolute_ms: i64,
    expires_at: Instant,
    absolute_expires_at: Instant,
}
impl Snapshot {
    fn private_copy(&self) -> Self {
        Self {
            incarnation: self.incarnation,
            lineage: self.lineage,
            subject: self.subject,
            digest: self.digest,
            generation: self.generation,
            idle_ms: self.idle_ms,
            absolute_ms: self.absolute_ms,
            expires_at: self.expires_at,
            absolute_expires_at: self.absolute_expires_at,
        }
    }
    // Only an actual checked DB snapshot can reach this parent-private bridge.
    // Effective idle expiry bounds request admission; absolute authority stays
    // immutable metadata and is never reconstructed from the shorter request gate.
    pub(super) fn into_verified_identity(self) -> Result<super::VerifiedIdentity, super::Failure> {
        if self.expires_at <= Instant::now() {
            return Err(super::Failure::expired());
        }
        if self.absolute_expires_at < self.expires_at {
            return Err(super::Failure::unavailable());
        }
        Ok(super::VerifiedIdentity {
            subject: self.subject,
            expires_at: self.expires_at,
            credential: super::CredentialMetadata {
                original_expires_at: self.absolute_expires_at,
                source: super::CredentialSource::Session(self),
            },
        })
    }
}
pub(super) struct Foundation {
    pool: sqlite::Pool,
    options: Options,
    incarnation: [u8; 32],
    clock: Clock,
    cookie_name: String,
    same_site: i64,
}
impl fmt::Debug for Foundation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionStoreFoundation")
            .finish_non_exhaustive()
    }
}
struct Count(i64);
impl FromRow for Count {
    fn columns() -> &'static [&'static str] {
        &["n"]
    }
    fn read(r: &crate::rusqlite::Row<'_>, i: &[usize]) -> crate::rusqlite::Result<Self> {
        Ok(Self(r.get(i[0])?))
    }
}
struct Schema {
    name: String,
    sql: String,
}
impl FromRow for Schema {
    fn columns() -> &'static [&'static str] {
        &["name", "sql"]
    }
    fn read(r: &crate::rusqlite::Row<'_>, i: &[usize]) -> crate::rusqlite::Result<Self> {
        Ok(Self {
            name: r.get(i[0])?,
            sql: r.get(i[1])?,
        })
    }
}
struct Metadata {
    version: i64,
    incarnation: Vec<u8>,
    last_wall: i64,
    next_lineage: i64,
    options: Options,
    cookie_name: String,
    same_site: i64,
}
impl FromRow for Metadata {
    fn columns() -> &'static [&'static str] {
        &[
            "version",
            "incarnation",
            "last_wall",
            "next_lineage",
            "max_live",
            "max_stored",
            "max_row_bytes",
            "cleanup_batch",
            "idle_ms",
            "absolute_ms",
            "touch_ms",
            "collision_attempts",
            "cookie_name",
            "same_site",
        ]
    }
    fn read(r: &crate::rusqlite::Row<'_>, i: &[usize]) -> crate::rusqlite::Result<Self> {
        Ok(Self {
            version: r.get(i[0])?,
            incarnation: r.get(i[1])?,
            last_wall: r.get(i[2])?,
            next_lineage: r.get(i[3])?,
            options: Options {
                max_live: r.get(i[4])?,
                max_stored: r.get(i[5])?,
                max_row_bytes: r.get(i[6])?,
                cleanup_batch: r.get(i[7])?,
                idle_ms: r.get(i[8])?,
                absolute_ms: r.get(i[9])?,
                touch_ms: r.get(i[10])?,
                collision_attempts: r.get(i[11])?,
            },
            cookie_name: r.get(i[12])?,
            same_site: r.get(i[13])?,
        })
    }
}
struct SessionRow {
    lineage: i64,
    subject: i64,
    digest: Vec<u8>,
    generation: i64,
    idle_ms: i64,
    absolute_ms: i64,
}
impl FromRow for SessionRow {
    fn columns() -> &'static [&'static str] {
        &[
            "lineage",
            "subject",
            "digest",
            "generation",
            "idle_ms",
            "absolute_ms",
        ]
    }
    fn read(r: &crate::rusqlite::Row<'_>, i: &[usize]) -> crate::rusqlite::Result<Self> {
        Ok(Self {
            lineage: r.get(i[0])?,
            subject: r.get(i[1])?,
            digest: r.get(i[2])?,
            generation: r.get(i[3])?,
            idle_ms: r.get(i[4])?,
            absolute_ms: r.get(i[5])?,
        })
    }
}
async fn metadata(tx: &sqlite::Tx) -> Result<Metadata, Failure> {
    sqlite::query(tx,sqlite::literal("SELECT version,incarnation,last_wall,next_lineage,max_live,max_stored,max_row_bytes,cleanup_batch,idle_ms,absolute_ms,touch_ms,collision_attempts,cookie_name,same_site FROM __nagi_session_meta WHERE id=1"),sqlite::parameters()).await.map_err(Failure::database)?.ok_or_else(Failure::invalid)
}
async fn exec(
    tx: &sqlite::Tx,
    sql: &'static str,
    params: sqlite::Parameters,
) -> Result<i64, Failure> {
    sqlite::exec(tx, sqlite::literal(sql), params)
        .await
        .map_err(Failure::database)
}
async fn finish<T>(
    tx: sqlite::Tx,
    result: Result<T, Failure>,
    after_write: Option<(&Foundation, i64)>,
) -> Result<T, Failure> {
    match result {
        Ok(value) => {
            sqlite::commit(tx).await.map_err(Failure::database)?;
            Ok(value)
        }
        Err(error) if error.outcome == sqlite::Outcome::NotApplicable && after_write.is_none() => {
            // This transaction contains maintenance only: no business write
            // was attempted. Retain its observed wall/finite cleanup on refusal.
            sqlite::commit(tx).await.map_err(Failure::database)?;
            Err(error)
        }
        Err(mut error) => {
            // No failed business mutation may survive, even if a later snapshot
            // failed with a policy classification rather than a database error.
            sqlite::rollback(tx).await.map_err(Failure::database)?;
            error.outcome = sqlite::Outcome::RolledBack;
            if let Some((store, observed_wall)) = after_write {
                // One bounded maintenance-only transaction, without retries.
                // Its database failure/unknown outcome must not be hidden.
                store.restore_maintenance(observed_wall).await?;
            }
            Err(error)
        }
    }
}
impl Foundation {
    // Private startup adapter, not the public Store/open/admission API. It uses
    // checked cookie configuration and the OS provider before the actual Tx path.
    pub(super) async fn initialize_with_os_secrets(
        pool: &sqlite::Pool,
        options: Options,
        cookie_name: &str,
        same_site: cookie::SameSite,
        clock: Clock,
    ) -> Result<Self, Failure> {
        let policy = secrets::CookiePolicy::new(cookie_name, same_site).map_err(Failure::secret)?;
        let incarnation = secrets::RawEntropy::generate().map_err(Failure::secret)?;
        Self::initialize(
            pool,
            options,
            policy.name(),
            policy.same_site_code(),
            incarnation.into_bytes(),
            clock,
        )
        .await
    }
    // Typed ID/digest connection into the existing private lookup primitive.
    // Request lease/admission and delivery are deliberately not claimed here.
    pub(super) async fn lookup_cookie(
        &self,
        cookie_headers: &[&str],
        authorization_present: bool,
    ) -> Result<Snapshot, Failure> {
        let same_site = match self.same_site {
            1 => cookie::SameSite::Strict,
            2 => cookie::SameSite::Lax,
            _ => return Err(Failure::invalid()),
        };
        let policy =
            secrets::CookiePolicy::new(&self.cookie_name, same_site).map_err(Failure::secret)?;
        let id = policy
            .read(cookie_headers, authorization_present)
            .map_err(Failure::secret)?;
        self.lookup(id.digest().into_bytes()).await
    }
    // Called only by a future secure Store initializer. This private layer
    // accepts already-reviewed Cookie policy and a fresh OS-CSPRNG incarnation.
    // This primitive attests the acquired Tx filename; the adapter above generates entropy.
    pub(super) async fn initialize(
        pool: &sqlite::Pool,
        options: Options,
        cookie_name: &str,
        same_site: i64,
        new_incarnation: [u8; 32],
        clock: Clock,
    ) -> Result<Self, Failure> {
        let tx = sqlite::begin(pool, sqlite::BeginMode::Immediate)
            .await
            .map_err(Failure::database)?;
        let result=async {
            // Inspect the acquired native Tx before any session DDL/read.
            if !tx.has_persistent_main() {return Err(Failure::invalid());}
            let existing=sqlite::all::<Schema>(&tx,sqlite::literal("SELECT name,sql FROM sqlite_schema WHERE name GLOB '__nagi_session_*' OR tbl_name GLOB '__nagi_session_*' ORDER BY name"),sqlite::parameters()).await.map_err(Failure::database)?;
            let sample=clock()?;Self::validate_clock(&sample)?;
            if existing.is_empty() {
                exec(&tx,META,sqlite::parameters()).await?;exec(&tx,ROWS,sqlite::parameters()).await?;exec(&tx,INDEX,sqlite::parameters()).await?;
                let mut p=sqlite::bind_bytes(sqlite::parameters(),new_incarnation.to_vec());p=sqlite::bind_i64(p,sample.ceil_ms);
                for n in [options.max_live,options.max_stored,options.max_row_bytes,options.cleanup_batch,options.idle_ms,options.absolute_ms,options.touch_ms,options.collision_attempts] {p=sqlite::bind_i64(p,n);}
                p=sqlite::bind_i64(sqlite::bind_text(p,cookie_name.to_owned()),same_site);
                exec(&tx,"INSERT INTO __nagi_session_meta VALUES(1,1,?,?,1,?,?,?,?,?,?,?,?,?,?)",p).await?;
            } else {
                let expected=[("__nagi_session_digest",INDEX),("__nagi_session_meta",META),("__nagi_session_rows",ROWS)];
                if existing.len()!=3 || existing.iter().zip(expected).any(|(row,(name,sql))|row.name!=name || row.sql!=sql) {return Err(Failure::invalid());}
            }
            let m=metadata(&tx).await?;
            if m.version!=1 || m.incarnation.len()!=32 || m.options!=options || m.cookie_name!=cookie_name || m.same_site!=same_site {return Err(Failure::invalid());}
            if sample.ceil_ms<m.last_wall {return Err(Failure::unavailable());}
            exec(&tx,"UPDATE __nagi_session_meta SET last_wall=? WHERE id=1",sqlite::bind_i64(sqlite::parameters(),sample.ceil_ms)).await?;
            let incarnation=m.incarnation.as_slice().try_into().map_err(|_|Failure::invalid())?;
            Ok(Self {pool:sqlite::clone_pool(pool),options,incarnation,clock,cookie_name:cookie_name.to_owned(),same_site})
        }.await;
        // Startup failures never adopt or mutate a conflicting schema/config.
        match result {
            Ok(store) => {
                sqlite::commit(tx).await.map_err(Failure::database)?;
                Ok(store)
            }
            Err(error) => {
                sqlite::rollback(tx).await.map_err(Failure::database)?;
                Err(error)
            }
        }
    }
    fn validate_clock(c: &ClockSample) -> Result<(), Failure> {
        if c.floor_ms < 0
            || c.ceil_ms < c.floor_ms
            || c.ceil_ms - c.floor_ms > 1
            || c.monotonic > c.monotonic_after
            || c.monotonic_after > Instant::now()
        {
            Err(Failure::unavailable())
        } else {
            Ok(())
        }
    }
    async fn transaction(&self) -> Result<(sqlite::Tx, ClockSample, Metadata), Failure> {
        let tx = sqlite::begin(&self.pool, sqlite::BeginMode::Immediate)
            .await
            .map_err(Failure::database)?;
        let result = async {
            if !tx.has_persistent_main() {
                return Err(Failure::unavailable());
            }
            // Sample after Immediate acquisition; UTC is bracketed by monotonic readings.
            let c = (self.clock)()?;
            Self::validate_clock(&c)?;
            let m = metadata(&tx).await?;
            if m.version != 1
                || m.incarnation.as_slice() != self.incarnation
                || m.options != self.options
                || m.cookie_name != self.cookie_name
                || m.same_site != self.same_site
            {
                return Err(Failure::unavailable());
            }
            if c.ceil_ms < m.last_wall {
                return Err(Failure::unavailable());
            }
            self.write_maintenance(&tx, c.ceil_ms).await?;
            Ok((c, m))
        }
        .await;
        match result {
            Ok((c, m)) => Ok((tx, c, m)),
            Err(error) => {
                sqlite::rollback(tx).await.map_err(Failure::database)?;
                Err(error)
            }
        }
    }
    async fn write_maintenance(&self, tx: &sqlite::Tx, wall: i64) -> Result<(), Failure> {
        exec(
            tx,
            "UPDATE __nagi_session_meta SET last_wall=? WHERE id=1",
            sqlite::bind_i64(sqlite::parameters(), wall),
        )
        .await?;
        exec(
            tx,
            "DELETE FROM __nagi_session_rows WHERE lineage IN (SELECT lineage FROM __nagi_session_rows WHERE idle_ms<=? OR absolute_ms<=? ORDER BY lineage LIMIT ?)",
            sqlite::bind_i64(sqlite::bind_i64(sqlite::bind_i64(sqlite::parameters(), wall), wall), self.options.cleanup_batch),
        ).await?;
        Ok(())
    }
    async fn restore_maintenance(&self, observed_wall: i64) -> Result<(), Failure> {
        let tx = sqlite::begin(&self.pool, sqlite::BeginMode::Immediate)
            .await
            .map_err(Failure::database)?;
        let result = async {
            if !tx.has_persistent_main() {
                return Err(Failure::unavailable());
            }
            let m = metadata(&tx).await?;
            if m.version != 1
                || m.incarnation.as_slice() != self.incarnation
                || m.options != self.options
                || m.cookie_name != self.cookie_name
                || m.same_site != self.same_site
            {
                return Err(Failure::unavailable());
            }
            // Preserve the already validated observation after rollback, even
            // if another transaction advanced the persisted clock meanwhile.
            // No new authority is issued by this maintenance-only transaction.
            self.write_maintenance(&tx, m.last_wall.max(observed_wall))
                .await
        }
        .await;
        match result {
            Ok(()) => sqlite::commit(tx).await.map_err(Failure::database),
            Err(mut error) => {
                sqlite::rollback(tx).await.map_err(Failure::database)?;
                error.outcome = sqlite::Outcome::RolledBack;
                Err(error)
            }
        }
    }
    fn snapshot(&self, row: SessionRow, c: &ClockSample) -> Result<Snapshot, Failure> {
        let remaining = row
            .idle_ms
            .min(row.absolute_ms)
            .checked_sub(c.ceil_ms)
            .filter(|n| *n > 0)
            .ok_or_else(Failure::credential)?;
        let expires_at = c
            .monotonic
            .checked_add(Duration::from_millis(remaining as u64))
            .ok_or_else(Failure::unavailable)?;
        if expires_at <= Instant::now() {
            return Err(Failure::credential());
        }
        // Project both immutable deadlines from the same conservative lookup
        // sample. Never use a later request budget to recreate absolute authority.
        let absolute_remaining = row
            .absolute_ms
            .checked_sub(c.ceil_ms)
            .filter(|n| *n > 0)
            .ok_or_else(Failure::credential)?;
        let absolute_expires_at = c
            .monotonic
            .checked_add(Duration::from_millis(absolute_remaining as u64))
            .ok_or_else(Failure::unavailable)?;
        Ok(Snapshot {
            incarnation: self.incarnation,
            lineage: row.lineage,
            subject: row.subject,
            digest: row
                .digest
                .as_slice()
                .try_into()
                .map_err(|_| Failure::unavailable())?,
            generation: row.generation,
            idle_ms: row.idle_ms,
            absolute_ms: row.absolute_ms,
            expires_at,
            absolute_expires_at,
        })
    }
    pub(super) async fn insert(
        &self,
        subject: i64,
        digest: [u8; 32],
        credential_expires: Instant,
    ) -> Result<Snapshot, Failure> {
        mutation::write::insert_primitive(self, subject, digest, credential_expires).await
    }

    pub(super) async fn lookup(&self, digest: [u8; 32]) -> Result<Snapshot, Failure> {
        let (tx, c, _) = self.transaction().await?;
        let result=async {
            let row=sqlite::query::<SessionRow>(&tx,sqlite::literal("SELECT lineage,subject,digest,generation,idle_ms,absolute_ms FROM __nagi_session_rows WHERE digest=? AND idle_ms>? AND absolute_ms>?"),sqlite::bind_i64(sqlite::bind_i64(sqlite::bind_bytes(sqlite::parameters(),digest.to_vec()),c.ceil_ms),c.ceil_ms)).await.map_err(Failure::database)?.ok_or_else(Failure::credential)?;
            let snapshot=self.snapshot(row,&c)?;
            let next=c.floor_ms.checked_add(self.options.idle_ms).ok_or_else(Failure::unavailable)?.min(snapshot.absolute_ms);
            let last_touch=snapshot.idle_ms.checked_sub(self.options.idle_ms).ok_or_else(Failure::unavailable)?;
            if c.floor_ms.checked_sub(last_touch).ok_or_else(Failure::unavailable)? >= self.options.touch_ms && next>snapshot.idle_ms {
                let p=sqlite::bind_i64(sqlite::bind_bytes(sqlite::bind_i64(sqlite::bind_i64(sqlite::parameters(),next),snapshot.lineage),digest.to_vec()),snapshot.generation);
                if exec(&tx,"UPDATE __nagi_session_rows SET idle_ms=? WHERE lineage=? AND digest=? AND generation=?",p).await?!=1 {return Err(Failure::denied());}
            }
            Ok(snapshot) // preserve the pre-touch authority of this lookup.
        }.await;
        finish(tx, result, None).await
    }
    pub(super) async fn rotate(
        &self,
        old: &Snapshot,
        digest: [u8; 32],
    ) -> Result<Snapshot, Failure> {
        mutation::write::rotate_primitive(self, old, digest).await
    }

    pub(super) async fn logout(&self, old: &Snapshot) -> Result<(), Failure> {
        let (tx, _, _) = self.transaction().await?;
        let result = async {
            if old.incarnation != self.incarnation {
                return Err(Failure::denied());
            }
            let present = sqlite::query::<Count>(
                &tx,
                sqlite::literal(
                    "SELECT count(*) AS n FROM __nagi_session_rows WHERE lineage=? AND subject<>?",
                ),
                sqlite::bind_i64(
                    sqlite::bind_i64(sqlite::parameters(), old.lineage),
                    old.subject,
                ),
            )
            .await
            .map_err(Failure::database)?
            .ok_or_else(Failure::unavailable)?
            .0;
            if present != 0 {
                return Err(Failure::denied());
            }
            exec(
                &tx,
                "DELETE FROM __nagi_session_rows WHERE lineage=? AND subject=?",
                sqlite::bind_i64(
                    sqlite::bind_i64(sqlite::parameters(), old.lineage),
                    old.subject,
                ),
            )
            .await?;
            Ok(()) // only returned after finish confirms this same Tx commit.
        }
        .await;
        finish(tx, result, None).await
    }
}
