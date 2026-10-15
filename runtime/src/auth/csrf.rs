//! Authenticated, reusable, finite Session-bound CSRF ledger.
//! A data handle never carries token material or grants authority.
use super::{session, FailureKind, Lease, LeaseOwner};
use crate::{sqlite, FromRow};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};
use subtle::{ConditionallySelectable, ConstantTimeEq};
#[cfg(test)]
#[path = "csrf/same_queue_probe.rs"]
mod same_queue_probe;
#[cfg(test)]
use same_queue_probe::Phase as CheckpointPhase;

const ROW_BYTES: i64 = 88;
const META: &str = "CREATE TABLE __nagi_csrf_meta(id INTEGER PRIMARY KEY CHECK(id=1),version INTEGER NOT NULL,incarnation BLOB NOT NULL CHECK(length(incarnation)=32),session_incarnation BLOB NOT NULL CHECK(length(session_incarnation)=32),max_stored INTEGER NOT NULL,max_payload_bytes INTEGER NOT NULL,cleanup_batch INTEGER NOT NULL,ttl_ms INTEGER NOT NULL,collision_attempts INTEGER NOT NULL,origin TEXT NOT NULL,last_wall INTEGER NOT NULL CHECK(last_wall>=0)) STRICT";
const ROWS: &str = "CREATE TABLE __nagi_csrf_tokens(digest BLOB PRIMARY KEY CHECK(length(digest)=32),session_incarnation BLOB NOT NULL CHECK(length(session_incarnation)=32),lineage INTEGER NOT NULL CHECK(lineage>0),generation INTEGER NOT NULL CHECK(generation>=0),expiry_ms INTEGER NOT NULL CHECK(expiry_ms>0)) STRICT, WITHOUT ROWID";
const INDEX: &str = "CREATE INDEX __nagi_csrf_expiry ON __nagi_csrf_tokens(expiry_ms,digest)";

pub struct Failure {
    kind: FailureKind,
    outcome: sqlite::Outcome,
}
impl Failure {
    fn new(kind: FailureKind) -> Self {
        Self {
            kind,
            outcome: sqlite::Outcome::NotApplicable,
        }
    }
    fn invalid() -> Self {
        Self::new(FailureKind::InvalidRequest)
    }
    fn unavailable() -> Self {
        Self::new(FailureKind::Unavailable)
    }
    fn denied() -> Self {
        Self::new(FailureKind::Denied)
    }
    fn internal() -> Self {
        Self::new(FailureKind::Internal)
    }
    fn database(e: sqlite::Failure) -> Self {
        Self {
            kind: FailureKind::Unavailable,
            outcome: e.outcome,
        }
    }
    fn session(e: session::Failure) -> Self {
        Self {
            kind: session::kind(&e),
            outcome: session::outcome(&e),
        }
    }
    fn authority(e: super::Failure) -> Self {
        Self::new(e.kind())
    }
    pub(crate) fn http_status(&self) -> crate::http_server::Status {
        use crate::http_server::Status;
        match self.kind {
            FailureKind::InvalidRequest => Status::BAD_REQUEST,
            FailureKind::InvalidCredential | FailureKind::Denied => Status::FORBIDDEN,
            FailureKind::Expired => Status::GATEWAY_TIMEOUT,
            FailureKind::Unavailable => Status::SERVICE_UNAVAILABLE,
            FailureKind::Internal => Status::INTERNAL_SERVER_ERROR,
        }
    }
}
impl fmt::Debug for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CsrfFailure")
            .field("kind", &self.kind)
            .field("outcome", &self.outcome)
            .finish()
    }
}
pub fn kind(e: &Failure) -> FailureKind {
    e.kind
}
pub fn outcome(e: &Failure) -> sqlite::Outcome {
    e.outcome
}
pub fn message(e: &Failure) -> &str {
    match e.kind {
        FailureKind::InvalidRequest => "invalid security request",
        FailureKind::InvalidCredential | FailureKind::Denied => "CSRF checkpoint refused",
        FailureKind::Expired => "authority expired",
        FailureKind::Unavailable => "security service unavailable",
        FailureKind::Internal => "security service failure",
    }
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(message(self))
    }
}
impl std::error::Error for Failure {}

#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    max_stored: i64,
    max_payload_bytes: i64,
    cleanup_batch: i64,
    ttl_ms: i64,
    collision_attempts: i64,
}
pub fn options(
    max_stored: i64,
    max_payload_bytes: i64,
    cleanup_batch: i64,
    ttl_ms: i64,
    collision_attempts: i64,
) -> Result<Options, Failure> {
    if [
        max_stored,
        max_payload_bytes,
        cleanup_batch,
        ttl_ms,
        collision_attempts,
    ]
    .iter()
    .any(|n| *n <= 0 || usize::try_from(*n).is_err() || isize::try_from(*n).is_err())
        || max_stored.checked_add(1).is_none()
        || max_stored.checked_mul(ROW_BYTES).is_none()
        || max_payload_bytes < ROW_BYTES
        || cleanup_batch > max_stored
        || Instant::now()
            .checked_add(Duration::from_millis(ttl_ms as u64))
            .is_none()
    {
        return Err(Failure::invalid());
    }
    Ok(Options {
        max_stored,
        max_payload_bytes,
        cleanup_batch,
        ttl_ms,
        collision_attempts,
    })
}
/// Explicitly shared immutable data handle; no raw parts, implicit Clone, or secret getter.
///
/// ```compile_fail
/// let _: fn(&nagi_runtime::auth::csrf::CsrfStore) -> nagi_runtime::auth::csrf::CsrfStore = Clone::clone;
/// ```
/// ```compile_fail
/// let _ = nagi_runtime::auth::csrf::CsrfStore { inner: () };
/// ```
pub struct CsrfStore {
    inner: Arc<Inner>,
}
impl fmt::Debug for CsrfStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CsrfStore").finish_non_exhaustive()
    }
}
pub fn clone_store(store: &CsrfStore) -> CsrfStore {
    CsrfStore {
        inner: Arc::clone(&store.inner),
    }
}
pub(crate) struct Inner {
    native: sqlite::Pool,
    context: session::PreloginContext,
    options: Options,
    incarnation: [u8; 32],
    origin: String,
    #[cfg(test)]
    reserve_wait: Option<Arc<ReserveWait>>,
    #[cfg(test)]
    postcommit_wait: Option<Arc<ReserveWait>>,
    #[cfg(test)]
    insert_enqueues: std::sync::atomic::AtomicUsize,
    #[cfg(test)]
    checkpoint_probe: Option<Arc<same_queue_probe::Probe>>,
}
#[cfg(test)]
struct ReserveWait {
    arrived: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
#[cfg(test)]
impl ReserveWait {
    fn new() -> Self {
        Self {
            arrived: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        }
    }
}
impl CsrfStore {
    pub(crate) fn into_inner(self) -> Arc<Inner> {
        self.inner
    }
    pub(crate) fn matches_store(&self, store: &session::Store) -> bool {
        self.inner.context.matches_store(store)
    }
}
struct Schema {
    name: String,
    sql: Option<String>,
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
    session_incarnation: Vec<u8>,
    options: Options,
    origin: String,
    last_wall: i64,
}
impl FromRow for Metadata {
    fn columns() -> &'static [&'static str] {
        &[
            "version",
            "incarnation",
            "session_incarnation",
            "max_stored",
            "max_payload_bytes",
            "cleanup_batch",
            "ttl_ms",
            "collision_attempts",
            "origin",
            "last_wall",
        ]
    }
    fn read(r: &crate::rusqlite::Row<'_>, i: &[usize]) -> crate::rusqlite::Result<Self> {
        Ok(Self {
            version: r.get(i[0])?,
            incarnation: r.get(i[1])?,
            session_incarnation: r.get(i[2])?,
            options: Options {
                max_stored: r.get(i[3])?,
                max_payload_bytes: r.get(i[4])?,
                cleanup_batch: r.get(i[5])?,
                ttl_ms: r.get(i[6])?,
                collision_attempts: r.get(i[7])?,
            },
            origin: r.get(i[8])?,
            last_wall: r.get(i[9])?,
        })
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
struct TokenRow {
    digest: Vec<u8>,
    expiry_ms: i64,
}
impl FromRow for TokenRow {
    fn columns() -> &'static [&'static str] {
        &["digest", "expiry_ms"]
    }
    fn read(r: &crate::rusqlite::Row<'_>, i: &[usize]) -> crate::rusqlite::Result<Self> {
        Ok(Self {
            digest: r.get(i[0])?,
            expiry_ms: r.get(i[1])?,
        })
    }
}
async fn schema(tx: &sqlite::Tx) -> Result<Vec<Schema>, Failure> {
    sqlite::all(tx,sqlite::literal("SELECT name,sql FROM sqlite_schema WHERE name GLOB '__nagi_csrf_*' OR tbl_name GLOB '__nagi_csrf_*' ORDER BY name"),sqlite::parameters()).await.map_err(Failure::database)
}
fn exact_schema(rows: &[Schema]) -> bool {
    let expected = [
        ("__nagi_csrf_expiry", INDEX),
        ("__nagi_csrf_meta", META),
        ("__nagi_csrf_tokens", ROWS),
    ];
    rows.len() == expected.len()
        && rows
            .iter()
            .zip(expected)
            .all(|(r, (n, s))| r.name == n && r.sql.as_deref() == Some(s))
}
async fn metadata(tx: &sqlite::Tx) -> Result<Metadata, Failure> {
    sqlite::query(tx,sqlite::literal("SELECT version,incarnation,session_incarnation,max_stored,max_payload_bytes,cleanup_batch,ttl_ms,collision_attempts,origin,last_wall FROM __nagi_csrf_meta WHERE id=1"),sqlite::parameters()).await.map_err(Failure::database)?.ok_or_else(Failure::unavailable)
}
fn checked_meta(m: &Metadata, context: &session::PreloginContext) -> Result<i64, Failure> {
    let o = options(
        m.options.max_stored,
        m.options.max_payload_bytes,
        m.options.cleanup_batch,
        m.options.ttl_ms,
        m.options.collision_attempts,
    )
    .map_err(|_| Failure::unavailable())?;
    if m.version != 1
        || m.incarnation.len() != 32
        || !context.matches_incarnation(&m.session_incarnation)
        || m.last_wall < 0
        || !crate::http_server::BoundOrigin::is_canonical(&m.origin)
    {
        return Err(Failure::unavailable());
    }
    o.max_stored
        .checked_mul(ROW_BYTES)
        .ok_or_else(Failure::unavailable)
}
// Optional closed ledger parser used by BOTH startup orders. No unchecked SQL cast.
pub(super) async fn declared_bytes(
    tx: &sqlite::Tx,
    context: &session::PreloginContext,
) -> Result<i64, Failure> {
    let rows = schema(tx).await?;
    if rows.is_empty() {
        return Ok(0);
    }
    if !exact_schema(&rows) {
        return Err(Failure::unavailable());
    }
    checked_meta(&metadata(tx).await?, context)
}
pub async fn open(
    store: &session::Store,
    options: Options,
    http: &crate::http_server::Options,
) -> Result<CsrfStore, Failure> {
    let origin =
        crate::http_server::checked_prelogin_origin(http).map_err(|_| Failure::invalid())?;
    let (native, context) = store.csrf_context().map_err(Failure::session)?;
    let tx = sqlite::begin(&native, sqlite::BeginMode::Immediate)
        .await
        .map_err(Failure::database)?;
    let mut wall = None;
    let result=async {
        context.verify(&tx,&mut wall).await.map_err(Failure::session)?;
        let prelogin=super::prelogin::declared_bytes(&tx,&context).await.map_err(|_|Failure::unavailable())?;
        context.reserved_bytes().and_then(|n|n.checked_add(prelogin)).and_then(|n|n.checked_add(options.max_stored.checked_mul(ROW_BYTES)?)).ok_or_else(Failure::invalid)?;
        let rows=schema(&tx).await?;
        let c=session::PreloginContext::track_sample(&mut wall,context.sample().map_err(Failure::session)?,0).map_err(Failure::session)?;
        if rows.is_empty() {
            for sql in [META,ROWS,INDEX] { sqlite::exec(&tx,sqlite::literal(sql),sqlite::parameters()).await.map_err(Failure::database)?; }
            let mut incarnation=[0;32]; getrandom::fill(&mut incarnation).map_err(|_|Failure::unavailable())?;
            let p=sqlite::bind_bytes(sqlite::parameters(),incarnation.to_vec()); let p=context.bind_incarnation(p);
            let p=sqlite::bind_i64(p,options.max_stored); let p=sqlite::bind_i64(p,options.max_payload_bytes); let p=sqlite::bind_i64(p,options.cleanup_batch); let p=sqlite::bind_i64(p,options.ttl_ms); let p=sqlite::bind_i64(p,options.collision_attempts); let p=sqlite::bind_text(p,origin.clone()); let p=sqlite::bind_i64(p,c.ceil_ms);
            sqlite::exec(&tx,sqlite::literal("INSERT INTO __nagi_csrf_meta(id,version,incarnation,session_incarnation,max_stored,max_payload_bytes,cleanup_batch,ttl_ms,collision_attempts,origin,last_wall) VALUES(1,1,?,?,?,?,?,?,?,?,?)"),p).await.map_err(Failure::database)?;
            Ok(incarnation)
        } else {
            if !exact_schema(&rows) { return Err(Failure::unavailable()); }
            let m=metadata(&tx).await?; checked_meta(&m,&context)?;
            session::PreloginContext::track_sample(&mut wall,c,m.last_wall).map_err(Failure::session)?;
            if m.options!=options || m.origin!=origin { return Err(Failure::unavailable()); }
            m.incarnation.try_into().map_err(|_|Failure::unavailable())
        }
    }.await;
    let result = match result {
        Ok(v) => sqlite::exec(
            &tx,
            sqlite::literal("UPDATE __nagi_csrf_meta SET last_wall=max(last_wall,?) WHERE id=1"),
            sqlite::bind_i64(sqlite::parameters(), wall.ok_or_else(Failure::unavailable)?),
        )
        .await
        .map(|_| v)
        .map_err(Failure::database),
        Err(e) => Err(e),
    };
    let incarnation = match result {
        Ok(v) => {
            sqlite::commit(tx).await.map_err(Failure::database)?;
            v
        }
        Err(mut e) => {
            sqlite::rollback(tx).await.map_err(Failure::database)?;
            e.outcome = sqlite::Outcome::RolledBack;
            return Err(e);
        }
    };
    Ok(CsrfStore {
        inner: Arc::new(Inner {
            native,
            context,
            options,
            incarnation,
            origin,
            #[cfg(test)]
            reserve_wait: None,
            #[cfg(test)]
            postcommit_wait: None,
            #[cfg(test)]
            insert_enqueues: std::sync::atomic::AtomicUsize::new(0),
            #[cfg(test)]
            checkpoint_probe: None,
        }),
    })
}

pub(crate) fn constrain(lease: &Lease, until: Instant) -> Result<(), Failure> {
    let mut gate = lease.checked_gate().map_err(Failure::authority)?;
    gate.expires_at = gate.expires_at.min(until);
    if gate.expires_at <= Instant::now() {
        return Err(Failure::new(FailureKind::Expired));
    }
    Ok(())
}
pub(crate) fn live_deadline(lease: &Lease) -> Result<Instant, Failure> {
    let gate = lease.checked_gate().map_err(Failure::authority)?;
    Ok(gate.expires_at)
}
pub(crate) fn token_digest(headers: &hyper::HeaderMap) -> Result<[u8; 32], Failure> {
    let mut values = headers.get_all("x-csrf-token").iter();
    let value = values.next().ok_or_else(Failure::denied)?;
    if values.next().is_some() {
        return Err(Failure::invalid());
    }
    let value = value.to_str().map_err(|_| Failure::denied())?;
    if value.len() != 43 {
        return Err(Failure::denied());
    }
    let mut bytes = [0; 33];
    let n = URL_SAFE_NO_PAD
        .decode_slice(value, &mut bytes)
        .map_err(|_| Failure::denied())?;
    if n != 32 || URL_SAFE_NO_PAD.encode(&bytes[..n]) != value {
        return Err(Failure::denied());
    }
    Ok(Sha256::digest(&bytes[..n]).into())
}
fn fresh_token() -> Result<(String, [u8; 32]), Failure> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|_| Failure::unavailable())?;
    Ok((URL_SAFE_NO_PAD.encode(bytes), Sha256::digest(bytes).into()))
}
#[derive(PartialEq, Eq)]
enum Phase {
    Empty,
    Pending,
    Ready,
    Sealed,
    Terminal,
}
struct Cell {
    phase: Phase,
    token: Option<String>,
    context: Option<[u8; 32]>,
    expiry: Option<i64>,
    deadline: Option<Instant>,
    wall: Option<i64>,
}
pub(crate) struct TokenOwner {
    pub(crate) lease: LeaseOwner,
    cell: Arc<Mutex<Cell>>,
}
pub(crate) struct TokenSeal {
    request: u64,
    cell: Weak<Mutex<Cell>>,
    context: [u8; 32],
}
impl TokenOwner {
    pub(crate) fn new(lease: LeaseOwner) -> Self {
        Self {
            lease,
            cell: Arc::new(Mutex::new(Cell {
                phase: Phase::Empty,
                token: None,
                context: None,
                expiry: None,
                deadline: None,
                wall: None,
            })),
        }
    }
    fn clear(&self) {
        let mut c = self.cell.lock().unwrap_or_else(|e| e.into_inner());
        c.token = None;
        c.phase = Phase::Terminal;
        c.context = None;
        c.expiry = None;
        c.deadline = None;
        c.wall = None;
    }
    pub(crate) fn retire(&self) {
        let _gate = self
            .lease
            .lease
            .gate
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        self.clear();
    }
    fn stage(&self, token: String) -> Result<(), Failure> {
        let _gate = self
            .lease
            .lease
            .checked_gate()
            .map_err(Failure::authority)?;
        let mut c = self.cell.lock().map_err(|_| Failure::internal())?;
        if !matches!(c.phase, Phase::Empty | Phase::Pending) {
            return Err(Failure::internal());
        }
        c.token = Some(token);
        c.phase = Phase::Pending;
        Ok(())
    }
    fn ready(
        &self,
        context: [u8; 32],
        expiry: i64,
        deadline: Instant,
        wall: i64,
    ) -> Result<(), Failure> {
        let _gate = self
            .lease
            .lease
            .checked_gate()
            .map_err(Failure::authority)?;
        let mut c = self.cell.lock().map_err(|_| Failure::internal())?;
        if c.phase != Phase::Pending || c.token.is_none() {
            return Err(Failure::internal());
        }
        if deadline <= Instant::now() || wall >= expiry {
            return Err(Failure::denied());
        }
        c.phase = Phase::Ready;
        c.context = Some(context);
        c.expiry = Some(expiry);
        c.deadline = Some(deadline);
        c.wall = Some(wall);
        Ok(())
    }
    pub(crate) fn seal(&self) -> Result<TokenSeal, Failure> {
        let _gate = self
            .lease
            .lease
            .checked_gate()
            .map_err(Failure::authority)?;
        let mut c = self.cell.lock().map_err(|_| Failure::internal())?;
        if c.phase != Phase::Ready {
            return Err(Failure::internal());
        }
        let context = c.context.ok_or_else(Failure::internal)?;
        c.phase = Phase::Sealed;
        Ok(TokenSeal {
            request: self.lease.lease.id,
            cell: Arc::downgrade(&self.cell),
            context,
        })
    }
}
impl Drop for TokenOwner {
    fn drop(&mut self) {
        self.retire();
    }
}
struct Pending<'a>(&'a TokenOwner, bool);
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if self.1 {
            self.0.retire();
        }
    }
}

impl Inner {
    async fn issue_reservation<'a>(
        &self,
        tx: &'a sqlite::Tx,
    ) -> Result<sqlite::ExecReservation<'a>, Failure> {
        #[cfg(test)]
        if let Some(wait) = &self.reserve_wait {
            // Fixture queue is exactly two: prove that the actual reservation
            // waits, without holding a reservation across a second SQL send.
            let first = tx.reserve_exec().await.map_err(Failure::database)?;
            let second = tx.reserve_exec().await.map_err(Failure::database)?;
            let mut pending = Box::pin(tx.reserve_exec());
            assert!(futures_util::poll!(&mut pending).is_pending());
            wait.arrived.notify_one();
            wait.release.notified().await;
            drop(first);
            drop(second);
            return pending.await.map_err(Failure::database);
        }
        tx.reserve_exec().await.map_err(Failure::database)
    }
    pub(crate) fn bound_to(&self, origin: &crate::http_server::BoundOrigin) -> bool {
        origin.matches(&self.origin)
    }
    pub(crate) async fn identity(
        &self,
        headers: &hyper::HeaderMap,
    ) -> Result<super::VerifiedIdentity, super::Failure> {
        self.context.lookup_http_identity(headers).await
    }
    fn clock(&self, wall: &mut Option<i64>) -> Result<session::PreloginClock, Failure> {
        session::PreloginContext::track_sample(
            wall,
            self.context.sample().map_err(Failure::session)?,
            0,
        )
        .map_err(Failure::session)
    }
    async fn check(&self, tx: &sqlite::Tx, wall: &mut Option<i64>) -> Result<(), Failure> {
        let c = self
            .context
            .verify(tx, wall)
            .await
            .map_err(Failure::session)?;
        if !exact_schema(&schema(tx).await?) {
            return Err(Failure::unavailable());
        }
        let m = metadata(tx).await?;
        if m.version != 1
            || m.incarnation.as_slice() != self.incarnation
            || !self.context.matches_incarnation(&m.session_incarnation)
            || m.options != self.options
            || m.origin != self.origin
            || m.last_wall < 0
        {
            return Err(Failure::unavailable());
        }
        session::PreloginContext::track_sample(wall, c, m.last_wall).map_err(Failure::session)?;
        Ok(())
    }
    async fn maintenance(&self, tx: &sqlite::Tx, wall: i64, cleanup: bool) -> Result<(), Failure> {
        if cleanup {
            let p = sqlite::bind_i64(
                sqlite::bind_i64(sqlite::parameters(), wall),
                self.options.cleanup_batch,
            );
            sqlite::exec(tx,sqlite::literal("DELETE FROM __nagi_csrf_tokens WHERE digest IN (SELECT digest FROM __nagi_csrf_tokens WHERE expiry_ms<=? ORDER BY expiry_ms,digest LIMIT ?)"),p).await.map_err(Failure::database)?;
        }
        sqlite::exec(
            tx,
            sqlite::literal("UPDATE __nagi_csrf_meta SET last_wall=max(last_wall,?) WHERE id=1"),
            sqlite::bind_i64(sqlite::parameters(), wall),
        )
        .await
        .map_err(Failure::database)?;
        Ok(())
    }
    async fn restore(&self, wall: &mut Option<i64>, cleanup: bool) -> Result<(), Failure> {
        let Some(observed) = *wall else {
            return Ok(());
        };
        let tx = sqlite::begin(&self.native, sqlite::BeginMode::Immediate)
            .await
            .map_err(Failure::database)?;
        let result = async {
            let mut current = None;
            self.check(&tx, &mut current).await?;
            self.maintenance(
                &tx,
                observed.max(current.ok_or_else(Failure::unavailable)?),
                cleanup,
            )
            .await
        }
        .await;
        match result {
            Ok(()) => sqlite::commit(tx).await.map_err(Failure::database),
            Err(mut e) => {
                sqlite::rollback(tx).await.map_err(Failure::database)?;
                e.outcome = sqlite::Outcome::RolledBack;
                Err(e)
            }
        }
    }
    async fn finish<T>(
        &self,
        tx: sqlite::Tx,
        result: Result<T, Failure>,
        wall: &mut Option<i64>,
    ) -> Result<T, Failure> {
        match result {
            Ok(v) => {
                sqlite::commit(tx).await.map_err(Failure::database)?;
                Ok(v)
            }
            Err(mut e) => {
                sqlite::rollback(tx).await.map_err(Failure::database)?;
                e.outcome = sqlite::Outcome::RolledBack;
                self.restore(wall, true).await?;
                Err(e)
            }
        }
    }
    pub(crate) async fn issue(
        &self,
        owner: &TokenOwner,
        b: &session::CsrfBinding,
    ) -> Result<(), Failure> {
        owner.lease.lease.validate().map_err(Failure::authority)?;
        let (token, mut digest) = fresh_token()?;
        owner.stage(token)?;
        let mut pending = Pending(owner, true);
        let tx = sqlite::begin(&self.native, sqlite::BeginMode::Immediate)
            .await
            .map_err(Failure::database)?;
        let mut wall = None;
        let result=async {
            self.check(&tx,&mut wall).await?;
            let c=self.clock(&mut wall)?;
            self.context.verify_binding(&tx,b,&c,&owner.lease.lease).await.map_err(Failure::session)?;
            self.maintenance(&tx,c.ceil_ms,true).await?;
            let count=sqlite::query::<Count>(&tx,sqlite::literal("SELECT count(*) AS n FROM __nagi_csrf_tokens"),sqlite::parameters()).await.map_err(Failure::database)?.ok_or_else(Failure::unavailable)?.0;
            if count<0 || count>=self.options.max_stored || count.checked_add(1).and_then(|n|n.checked_mul(ROW_BYTES)).is_none_or(|n|n>self.options.max_payload_bytes) { return Err(Failure::unavailable()); }
            let c=self.clock(&mut wall)?;
            b.check_clock(&c,&owner.lease.lease).map_err(Failure::session)?;
            let expiry=c.floor_ms.checked_add(self.options.ttl_ms).ok_or_else(Failure::unavailable)?.min(b.absolute_ms());
            let deadline=project(&c,expiry)?.min(b.absolute_deadline());
            for attempt in 0..self.options.collision_attempts {
                if attempt>0 { let (token,next)=fresh_token()?;owner.stage(token)?;digest=next; }
                let reserve=self.issue_reservation(&tx).await?;
                let fresh=self.clock(&mut wall)?; b.check_clock(&fresh,&owner.lease.lease).map_err(Failure::session)?;
                if fresh.ceil_ms>=expiry || Instant::now()>=deadline { return Err(Failure::denied()); }
                let p=sqlite::bind_bytes(sqlite::parameters(),digest.to_vec());let p=b.token_parameters(p);let p=sqlite::bind_i64(p,expiry);
                let p=self.context.bind_incarnation(p);let p=b.current_parameters(p,fresh.ceil_ms);
                let reply={ let _gate=owner.lease.lease.checked_gate().map_err(Failure::authority)?;
                    let reply=reserve.enqueue(sqlite::literal("INSERT OR IGNORE INTO __nagi_csrf_tokens(digest,session_incarnation,lineage,generation,expiry_ms) SELECT ?,?,?,?,? WHERE EXISTS(SELECT 1 FROM __nagi_session_meta WHERE id=1 AND incarnation=?) AND EXISTS(SELECT 1 FROM __nagi_session_rows WHERE lineage=? AND subject=? AND digest=? AND generation=? AND absolute_ms=? AND idle_ms>? AND absolute_ms>?)"),p);
                    #[cfg(test)] self.insert_enqueues.fetch_add(1,std::sync::atomic::Ordering::SeqCst);
                    reply };
                let affected=reply.await.map_err(Failure::database)?;
                if affected==1 {
                    let fresh=self.clock(&mut wall)?;b.check_clock(&fresh,&owner.lease.lease).map_err(Failure::session)?;
                    if fresh.ceil_ms>=expiry || Instant::now()>=deadline { return Err(Failure::denied()); }
                    self.maintenance(&tx,wall.ok_or_else(Failure::unavailable)?,false).await?;
                    return Ok((expiry,deadline));
                }
                if affected!=0 { return Err(Failure::unavailable()); }
            }
            Err(Failure::unavailable())
        }.await;
        let (expiry, deadline) = self.finish(tx, result, &mut wall).await?;
        #[cfg(test)]
        if let Some(wait) = &self.postcommit_wait {
            // Real business commit has already completed. This finite fixture
            // observes cancellation before Ready, not rollback from cancellation.
            wait.arrived.notify_one();
            wait.release.notified().await;
        }
        let final_result = async {
            let sampled = self.clock(&mut wall).and_then(|c| {
                b.check_clock(&c, &owner.lease.lease)
                    .map_err(Failure::session)?;
                Ok(c)
            });
            self.restore(&mut wall, false).await?;
            sampled?;
            let c = self.clock(&mut wall)?;
            b.check_clock(&c, &owner.lease.lease)
                .map_err(Failure::session)?;
            owner.ready(
                self.incarnation,
                expiry,
                deadline,
                wall.ok_or_else(Failure::unavailable)?,
            )
        }
        .await;
        final_result.map_err(|mut e| {
            e.outcome = sqlite::Outcome::Committed;
            e
        })?;
        pending.1 = false;
        Ok(())
    }
    pub(crate) async fn checkpoint(
        &self,
        lease: &Lease,
        b: &session::CsrfBinding,
        digest: [u8; 32],
    ) -> Result<(), Failure> {
        #[cfg(test)]
        self.checkpoint_phase(CheckpointPhase::EntryLease);
        lease.validate().map_err(Failure::authority)?;
        #[cfg(test)]
        self.checkpoint_phase(CheckpointPhase::Begin);
        let tx = sqlite::begin(&self.native, sqlite::BeginMode::Immediate)
            .await
            .map_err(Failure::database)?;
        let mut wall = None;
        let result=async {
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::Check);
            self.check(&tx,&mut wall).await?;
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::InitialSample);
            let c=self.clock(&mut wall)?;
            #[cfg(test)]
            self.checkpoint_sample(c.ceil_ms);
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::Binding);
            self.context.verify_binding(&tx,b,&c,lease).await.map_err(Failure::session)?;
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::Cleanup);
            self.maintenance(&tx,c.ceil_ms,true).await?;
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::ReadTokens);
            let p=b.token_parameters(sqlite::parameters());let p=sqlite::bind_i64(p,self.options.max_stored.checked_add(1).ok_or_else(Failure::unavailable)?);
            let rows=sqlite::all::<TokenRow>(&tx,sqlite::literal("SELECT digest,expiry_ms FROM __nagi_csrf_tokens WHERE session_incarnation=? AND lineage=? AND generation=? ORDER BY digest LIMIT ?"),p).await.map_err(Failure::database)?;
            if rows.len()>usize::try_from(self.options.max_stored).map_err(|_|Failure::unavailable())? { return Err(Failure::unavailable()); }
            let mut matched=subtle::Choice::from(0);let mut expiry=0i64;
            for row in rows {
                let candidate:[u8;32]=row.digest.try_into().map_err(|_|Failure::unavailable())?;
                if row.expiry_ms<=0 || row.expiry_ms>b.absolute_ms() { return Err(Failure::unavailable()); }
                let same=candidate.ct_eq(&digest);matched|=same;expiry=i64::conditional_select(&expiry,&row.expiry_ms,same);
            }
            #[cfg(test)]
            self.checkpoint_match(bool::from(matched), expiry);
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::FreshSample);
            let fresh=self.clock(&mut wall)?;
            #[cfg(test)]
            self.checkpoint_sample(fresh.ceil_ms);
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::FreshBinding);
            b.check_clock(&fresh,lease).map_err(Failure::session)?;
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::FreshLease);
            lease.validate().map_err(Failure::authority)?;
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::Matched);
            if !bool::from(matched) || fresh.ceil_ms>=expiry { return Err(Failure::denied()); }
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::Project);
            let deadline=project(&fresh,expiry)?.min(b.absolute_deadline());
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::Maintain);
            self.maintenance(&tx,wall.ok_or_else(Failure::unavailable)?,false).await?;
            Ok((expiry,deadline))
        }.await;
        #[cfg(test)]
        self.checkpoint_refusal(result.is_err());
        #[cfg(test)]
        self.checkpoint_phase(CheckpointPhase::Finish);
        let (expiry, deadline) = self.finish(tx, result, &mut wall).await?;
        let result = async {
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::PostSample);
            let sampled = self.clock(&mut wall);
            #[cfg(test)]
            self.checkpoint_sample_result(&sampled);
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::Restore);
            self.restore(&mut wall, false).await?;
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::PostSampleResult);
            sampled?;
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::FinalSample);
            let c = self.clock(&mut wall)?;
            #[cfg(test)]
            self.checkpoint_sample(c.ceil_ms);
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::FinalBinding);
            b.check_clock(&c, lease).map_err(Failure::session)?;
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::FinalLease);
            lease.validate().map_err(Failure::authority)?;
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::FinalExpiry);
            if c.ceil_ms >= expiry || Instant::now() >= deadline {
                #[cfg(test)]
                self.checkpoint_actual((c.ceil_ms < expiry).then_some(true));
                return Err(Failure::denied());
            }
            #[cfg(test)]
            self.checkpoint_actual(Some(false));
            #[cfg(test)]
            self.checkpoint_phase(CheckpointPhase::Done);
            Ok(())
        }
        .await;
        result.map_err(|mut e| {
            e.outcome = sqlite::Outcome::Committed;
            e
        })
    }
    pub(crate) fn take_wire(
        &self,
        owner: &TokenOwner,
        seal: TokenSeal,
        b: &session::CsrfBinding,
    ) -> Result<hyper::body::Bytes, Failure> {
        if seal.request != owner.lease.lease.id
            || seal.context != self.incarnation
            || !Weak::ptr_eq(&seal.cell, &Arc::downgrade(&owner.cell))
        {
            return Err(Failure::internal());
        }
        let sampled = self.context.sample().map_err(Failure::session)?;
        let (token, expiry, deadline, mut wall) = {
            let _gate = owner
                .lease
                .lease
                .checked_gate()
                .map_err(Failure::authority)?;
            let mut c = owner.cell.lock().map_err(|_| Failure::internal())?;
            if c.phase != Phase::Sealed || c.context != Some(self.incarnation) {
                return Err(Failure::internal());
            }
            let fresh = session::PreloginContext::track_sample(&mut c.wall, sampled, 0)
                .map_err(Failure::session)?;
            b.check_clock(&fresh, &owner.lease.lease)
                .map_err(Failure::session)?;
            let expiry = c.expiry.ok_or_else(Failure::internal)?;
            let deadline = c.deadline.ok_or_else(Failure::internal)?;
            if fresh.ceil_ms >= expiry || Instant::now() >= deadline {
                return Err(Failure::denied());
            }
            let token = c.token.take().ok_or_else(Failure::internal)?;
            c.phase = Phase::Terminal;
            (token, expiry, deadline, c.wall)
        };
        let body = format!("{{\"csrf_token\":\"{token}\"}}");
        if body.len() != 60 {
            return Err(Failure::internal());
        }
        let c = self.clock(&mut wall)?;
        b.check_clock(&c, &owner.lease.lease)
            .map_err(Failure::session)?;
        owner.lease.lease.validate().map_err(Failure::authority)?;
        if c.ceil_ms >= expiry || Instant::now() >= deadline {
            return Err(Failure::denied());
        }
        Ok(hyper::body::Bytes::from(body))
    }
}
fn project(c: &session::PreloginClock, expiry: i64) -> Result<Instant, Failure> {
    let remaining = expiry
        .checked_sub(c.ceil_ms)
        .filter(|n| *n > 0)
        .ok_or_else(Failure::denied)?;
    c.monotonic
        .checked_add(Duration::from_millis(remaining as u64))
        .ok_or_else(Failure::unavailable)
}
#[cfg(test)]
#[path = "csrf/tests.rs"]
mod tests;
