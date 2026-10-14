//! Checked pre-login startup, closed bootstrap and BrowserLogin HTTP dispatch.
//! Compiler integration remains deferred.
//! Secrets have one request owner; only digests cross asynchronous database waits.
#[cfg(all(test, nagi_auth_stage_diag))]
use super::stage_observer::{Event, Op, Stage, Trace};
use super::{session, CredentialSource, FailureKind, Lease, LeaseOwner, VerifiedIdentity};
use crate::{sqlite, FromRow};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    future::Future,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
mod browser;
mod http_bridge;
pub(crate) use browser::ClaimContext;
#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use http_bridge::open_for_http;
pub(crate) use http_bridge::{HttpOwner, HttpSeal};
const ROW_BYTES: i64 = 72;
const META:&str="CREATE TABLE __nagi_prelogin_meta(id INTEGER PRIMARY KEY CHECK(id=1),version INTEGER NOT NULL,incarnation BLOB NOT NULL CHECK(length(incarnation)=32),session_incarnation BLOB NOT NULL CHECK(length(session_incarnation)=32),config TEXT NOT NULL,last_wall INTEGER NOT NULL CHECK(last_wall>=0),window_start INTEGER NOT NULL CHECK(window_start>=0),issued INTEGER NOT NULL CHECK(issued>=0)) STRICT";
const ROWS:&str="CREATE TABLE __nagi_prelogin_ready(handle BLOB PRIMARY KEY NOT NULL CHECK(length(handle)=32),token BLOB NOT NULL CHECK(length(token)=32),expiry_ms INTEGER NOT NULL CHECK(expiry_ms>=0)) STRICT";
const INDEX: &str = "CREATE INDEX __nagi_prelogin_expiry ON __nagi_prelogin_ready(expiry_ms)";
// No private database error/cause, credential or path is formatted.
#[derive(Debug)]
enum FailureOrigin {
    Core,
    Authority,
    Verifier,
}
pub struct Failure {
    kind: FailureKind,
    outcome: sqlite::Outcome,
    origin: FailureOrigin,
    // Does not replace the actual Outcome of a subsequent maintenance Tx.
    business_committed: bool,
}
impl fmt::Debug for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreLoginFailure")
            .field("kind", &self.kind)
            .field("outcome", &self.outcome)
            .finish()
    }
}
/// Stable classification without a database cause or secret material.
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
/// Preserve the actual native outcome separately from the failure classification.
pub fn outcome(failure: &Failure) -> sqlite::Outcome {
    failure.outcome
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(message(self))
    }
}
impl std::error::Error for Failure {}
impl Failure {
    fn after_commit(mut self) -> Self {
        self.business_committed = true;
        // No additional DB failure: known main-body commit. Actual maintenance
        // Unknown/RolledBack/Committed outcomes are retained verbatim instead.
        if self.outcome == sqlite::Outcome::NotApplicable {
            self.outcome = sqlite::Outcome::Committed;
        }
        self
    }
    fn invalid() -> Self {
        Self {
            kind: FailureKind::InvalidRequest,
            outcome: sqlite::Outcome::NotApplicable,
            origin: FailureOrigin::Core,
            business_committed: false,
        }
    }
    fn unavailable() -> Self {
        Self {
            kind: FailureKind::Unavailable,
            outcome: sqlite::Outcome::NotApplicable,
            origin: FailureOrigin::Core,
            business_committed: false,
        }
    }
    fn denied() -> Self {
        Self {
            kind: FailureKind::Denied,
            outcome: sqlite::Outcome::NotApplicable,
            origin: FailureOrigin::Core,
            business_committed: false,
        }
    }
    fn expired() -> Self {
        Self {
            kind: FailureKind::Expired,
            outcome: sqlite::Outcome::NotApplicable,
            origin: FailureOrigin::Core,
            business_committed: false,
        }
    }
    fn database(e: sqlite::Failure) -> Self {
        Self {
            kind: FailureKind::Unavailable,
            outcome: e.outcome,
            origin: FailureOrigin::Core,
            business_committed: false,
        }
    }
    fn session(e: session::Failure) -> Self {
        Self {
            kind: session::kind(&e),
            outcome: session::outcome(&e),
            origin: FailureOrigin::Core,
            business_committed: false,
        }
    }
    fn authority(e: super::Failure) -> Self {
        Self {
            kind: super::kind(&e),
            outcome: sqlite::Outcome::NotApplicable,
            origin: FailureOrigin::Authority,
            business_committed: false,
        }
    }
    fn verifier(e: super::Failure) -> Self {
        Self {
            kind: super::kind(&e),
            outcome: sqlite::Outcome::NotApplicable,
            origin: FailureOrigin::Verifier,
            business_committed: false,
        }
    }
    fn rejected_identity() -> Self {
        Self {
            kind: FailureKind::InvalidCredential,
            outcome: sqlite::Outcome::NotApplicable,
            origin: FailureOrigin::Verifier,
            business_committed: false,
        }
    }
}
#[derive(PartialEq, Eq)]
pub struct Options {
    max_live: i64,
    max_stored: i64,
    max_payload_bytes: i64,
    cleanup_batch: i64,
    ttl_ms: i64,
    collision_attempts: i64,
    issuance_per_window: i64,
    window_ms: i64,
    max_json_depth: i64,
    max_json_nodes: i64,
}
impl fmt::Debug for Options {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreLoginOptions").finish_non_exhaustive()
    }
}
/// Check ten explicit finite budgets; there are no numeric defaults.
#[expect(
    clippy::too_many_arguments,
    reason = "adopted pre-login options has ten explicit bounded dimensions"
)]
pub fn options(
    max_live: i64,
    max_stored: i64,
    max_payload_bytes: i64,
    cleanup_batch: i64,
    ttl_ms: i64,
    collision_attempts: i64,
    issuance_per_window: i64,
    window_ms: i64,
    max_json_depth: i64,
    max_json_nodes: i64,
) -> Result<Options, Failure> {
    if [
        max_live,
        max_stored,
        max_payload_bytes,
        cleanup_batch,
        ttl_ms,
        collision_attempts,
        issuance_per_window,
        window_ms,
        max_json_depth,
        max_json_nodes,
    ]
    .iter()
    .any(|v| *v <= 0 || usize::try_from(*v).is_err())
        || max_live > max_stored
        || cleanup_batch > max_stored
        || max_payload_bytes < ROW_BYTES
        || max_stored.checked_mul(ROW_BYTES).is_none()
        || Instant::now()
            .checked_add(Duration::from_millis(ttl_ms as u64))
            .is_none()
    {
        return Err(Failure::invalid());
    }
    Ok(Options {
        max_live,
        max_stored,
        max_payload_bytes,
        cleanup_batch,
        ttl_ms,
        collision_attempts,
        issuance_per_window,
        window_ms,
        max_json_depth,
        max_json_nodes,
    })
}
pub struct CookieOptions {
    name: String,
    same_site: session::SameSite,
}
impl fmt::Debug for CookieOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreLoginCookieOptions")
            .finish_non_exhaustive()
    }
}
/// Checked host-only Secure/HttpOnly cookie profile; Strict or Lax only.
pub fn cookie_options(name: &str, same_site: session::SameSite) -> Result<CookieOptions, Failure> {
    // Reuse the checked Session host-only cookie grammar; pre-login is Strict/Lax only.
    if same_site == session::SameSite::None {
        return Err(Failure::invalid());
    }
    session::cookie_options(name, same_site).map_err(Failure::session)?;
    Ok(CookieOptions {
        name: name.to_owned(),
        same_site,
    })
}
pub(crate) struct Pool {
    native: sqlite::Pool,
    context: session::PreloginContext,
    options: Options,
    cookie: CookieOptions,
    incarnation: [u8; 32],
    config: String,
    origin: String,
    #[cfg(all(test, nagi_auth_stage_diag))]
    stage_slot: std::sync::atomic::AtomicU8,
    #[cfg(test)]
    business_enqueues: std::sync::atomic::AtomicUsize,
    #[cfg(test)]
    postcommit_wait: Option<Arc<PostcommitMaintenanceWait>>,
    #[cfg(test)]
    preconsume_wait: Option<Arc<PostcommitMaintenanceWait>>,
    #[cfg(test)]
    entropy_calls: std::sync::atomic::AtomicUsize,
}
/// Immutable data handle sharing one independently bounded native manager.
/// No proof, implicit Clone, raw pool, path, origin or secret getter.
pub struct PreLoginPool {
    inner: Arc<Pool>,
}
impl fmt::Debug for PreLoginPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreLoginPool").finish_non_exhaustive()
    }
}
/// Fork the Session store's checked persistent source and validate configuration
/// before success. HTTP options are borrowed only during open; their sealed
/// ExternalOrigin is checked before any native fork or PreLogin DDL.
pub async fn open(
    store: &session::Store,
    sql_options: sqlite::Options,
    options: Options,
    cookie: CookieOptions,
    http_options: &crate::http_server::Options,
) -> Result<PreLoginPool, Failure> {
    let pool =
        http_bridge::open_for_http(store, sql_options, options, cookie, http_options).await?;
    Ok(PreLoginPool {
        inner: Arc::new(pool),
    })
}
/// Explicitly clone the data handle; no request or authorization is cloned.
pub fn clone_pool(pool: &PreLoginPool) -> PreLoginPool {
    PreLoginPool {
        inner: Arc::clone(&pool.inner),
    }
}
/// Stop the shared anonymous manager and observe actual native join. Stop and
/// join HTTP first; Drop alone is not a close completion. Auth closes separately.
pub async fn close(pool: &PreLoginPool, timeout_ms: i64) -> Result<(), Failure> {
    close_native(&pool.inner, timeout_ms).await
}
pub(crate) fn into_http_pool(pool: PreLoginPool) -> Arc<Pool> {
    pool.inner
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
    config: String,
    last_wall: i64,
    window_start: i64,
    issued: i64,
}
impl FromRow for Metadata {
    fn columns() -> &'static [&'static str] {
        &[
            "version",
            "incarnation",
            "session_incarnation",
            "config",
            "last_wall",
            "window_start",
            "issued",
        ]
    }
    fn read(r: &crate::rusqlite::Row<'_>, i: &[usize]) -> crate::rusqlite::Result<Self> {
        Ok(Self {
            version: r.get(i[0])?,
            incarnation: r.get(i[1])?,
            session_incarnation: r.get(i[2])?,
            config: r.get(i[3])?,
            last_wall: r.get(i[4])?,
            window_start: r.get(i[5])?,
            issued: r.get(i[6])?,
        })
    }
}
struct Row {
    handle: Vec<u8>,
    token: Vec<u8>,
    expiry: i64,
}
impl FromRow for Row {
    fn columns() -> &'static [&'static str] {
        &["handle", "token", "expiry_ms"]
    }
    fn read(r: &crate::rusqlite::Row<'_>, i: &[usize]) -> crate::rusqlite::Result<Self> {
        Ok(Self {
            handle: r.get(i[0])?,
            token: r.get(i[1])?,
            expiry: r.get(i[2])?,
        })
    }
}
async fn metadata(tx: &sqlite::Tx) -> Result<Metadata, Failure> {
    sqlite::query(tx,sqlite::literal("SELECT version,incarnation,session_incarnation,config,last_wall,window_start,issued FROM __nagi_prelogin_meta WHERE id=1"),sqlite::parameters()).await.map_err(Failure::database)?.ok_or_else(Failure::unavailable)
}
pub(crate) async fn close_native(pool: &Pool, timeout_ms: i64) -> Result<(), Failure> {
    sqlite::close(&pool.native, timeout_ms)
        .await
        .map_err(Failure::database)
}
// Only trusted startup may supply origin here. Binding it to actual ExternalOrigin,
// not an app string/public factory, is a prerequisite of the later HTTP slice.
async fn open_bound(
    store: &session::Store,
    sql_options: sqlite::Options,
    options: Options,
    cookie: CookieOptions,
    origin: &str,
) -> Result<Pool, Failure> {
    let (native, context) = store.fork_prelogin(sql_options).map_err(Failure::session)?;
    let result = async {
        if cookie.name==context.cookie_name() || context.reserved_bytes().and_then(|v|v.checked_add(options.max_stored.checked_mul(ROW_BYTES)?)).is_none(){
            return Err(Failure::invalid());
        }
        let config=format!("{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}:{}|{}|{}:{}",options.max_live,options.max_stored,options.max_payload_bytes,options.cleanup_batch,options.ttl_ms,options.collision_attempts,options.issuance_per_window,options.window_ms,options.max_json_depth,options.max_json_nodes,cookie.name.len(),cookie.name,if cookie.same_site==session::SameSite::Strict {
            "Strict"}
        else{
            "Lax"}
        ,origin.len(),origin);
        let tx = sqlite::begin(&native,sqlite::BeginMode::Immediate).await.map_err(Failure::database)?;
        let mut wall=None;
        let result = async {
            context.verify(&tx,&mut wall).await.map_err(Failure::session)?;
            let b1=super::csrf::declared_bytes(&tx,&context).await.map_err(|_|Failure::unavailable())?;
            context.reserved_bytes().and_then(|n|n.checked_add(options.max_stored.checked_mul(ROW_BYTES)?)).and_then(|n|n.checked_add(b1)).ok_or_else(Failure::invalid)?;
            let schema=sqlite::all::<Schema>(&tx,sqlite::literal("SELECT name,sql FROM sqlite_schema WHERE (name GLOB '__nagi_prelogin_*' OR tbl_name GLOB '__nagi_prelogin_*') AND name NOT GLOB 'sqlite_autoindex_*' ORDER BY name"),sqlite::parameters()).await.map_err(Failure::database)?;
            let clock=session::PreloginContext::track_sample(&mut wall,context.sample().map_err(Failure::session)?,0).map_err(Failure::session)?;
            if schema.is_empty(){
                for sql in [META,ROWS,INDEX]{
                    sqlite::exec(&tx,sqlite::literal(sql),sqlite::parameters()).await.map_err(Failure::database)?;
                }
                let mut incarnation=[0;
                32];
                getrandom::fill(&mut incarnation).map_err(|_|Failure::unavailable())?;
                let p=sqlite::bind_bytes(sqlite::parameters(),incarnation.to_vec());
                let p=context.bind_incarnation(p);
                let p=sqlite::bind_text(p,config.clone());
                let p=sqlite::bind_i64(p,clock.ceil_ms);
                let p=sqlite::bind_i64(p,clock.floor_ms);
                sqlite::exec(&tx,sqlite::literal("INSERT INTO __nagi_prelogin_meta(id,version,incarnation,session_incarnation,config,last_wall,window_start,issued) VALUES(1,1,?,?,?,?,?,0)"),p).await.map_err(Failure::database)?;
                Ok(incarnation)
            }
            else{
                let expected=[("__nagi_prelogin_expiry",INDEX),("__nagi_prelogin_meta",META),("__nagi_prelogin_ready",ROWS)];
                if schema.len()!=expected.len() || schema.iter().zip(expected).any(|(s,(n,q))|s.name!=n||s.sql.as_deref()!=Some(q)){
                    return Err(Failure::unavailable());
                }
                let m=metadata(&tx).await?;
                let clock=session::PreloginContext::track_sample(&mut wall,clock,m.last_wall).map_err(Failure::session)?;
                if m.version!=1||m.config!=config||!context.matches_incarnation(&m.session_incarnation)||clock.ceil_ms<m.last_wall||m.window_start<0||m.issued<0||m.issued>options.issuance_per_window{
                    return Err(Failure::unavailable());
                }
                m.incarnation.try_into().map_err(|_|Failure::unavailable())
            }
        }
        .await;
        let result=match result {
            Ok(value)=>match sqlite::exec(&tx,sqlite::literal("UPDATE __nagi_prelogin_meta SET last_wall=max(last_wall,?) WHERE id=1"),sqlite::bind_i64(sqlite::parameters(),wall.ok_or_else(Failure::unavailable)?)).await {
                Ok(_)=>Ok(value),Err(error)=>Err(Failure::database(error))
            },
            Err(error)=>Err(error)
        };
        let incarnation=match result {
            Ok(v)=>{
                sqlite::commit(tx).await.map_err(Failure::database)?;
                v}
            ,Err(e)=>{
                sqlite::rollback(tx).await.map_err(Failure::database)?;
                return Err(e);
            }
        }
        ;
        Ok(Pool {
            native:sqlite::clone_pool(&native),context,options,cookie,incarnation,config,origin:origin.to_owned(),
            #[cfg(all(test, nagi_auth_stage_diag))] stage_slot:std::sync::atomic::AtomicU8::new(0),
            #[cfg(test)] business_enqueues:std::sync::atomic::AtomicUsize::new(0),
            #[cfg(test)] postcommit_wait:None,
            #[cfg(test)] preconsume_wait:None,
            #[cfg(test)] entropy_calls:std::sync::atomic::AtomicUsize::new(0)}
        )
    }
    .await;
    if result.is_err() {
        sqlite::close(&native, 1000)
            .await
            .map_err(Failure::database)?;
    }
    result
}
// No Clone/Debug/Serde or production getter on secret material.
struct Pair {
    handle: String,
    token: String,
}
fn fresh_pair() -> Result<(Pair, ClaimInput), Failure> {
    let mut h = [0; 32];
    let mut t = [0; 32];
    getrandom::fill(&mut h).map_err(|_| Failure::unavailable())?;
    getrandom::fill(&mut t).map_err(|_| Failure::unavailable())?;
    let input = ClaimInput {
        handle: Sha256::digest(h).into(),
        token: Sha256::digest(t).into(),
    };
    Ok((
        Pair {
            handle: URL_SAFE_NO_PAD.encode(h),
            token: URL_SAFE_NO_PAD.encode(t),
        },
        input,
    ))
}
struct ClaimInput {
    handle: [u8; 32],
    token: [u8; 32],
}
impl ClaimInput {
    fn parse(handle: &str, token: &str) -> Result<Self, Failure> {
        fn part(value: &str) -> Result<[u8; 32], Failure> {
            if value.len() != 43 {
                return Err(Failure::denied());
            }
            let mut b = [0; 33];
            let n = URL_SAFE_NO_PAD
                .decode_slice(value, &mut b)
                .map_err(|_| Failure::denied())?;
            if n != 32 || URL_SAFE_NO_PAD.encode(&b[..n]) != value {
                return Err(Failure::denied());
            }
            Ok(Sha256::digest(&b[..n]).into())
        }
        Ok(Self {
            handle: part(handle)?,
            token: part(token)?,
        })
    }
}
enum Phase {
    Vacant,
    Pending,
    Ready,
    Sealed,
    Terminal,
}
struct Cell {
    phase: Phase,
    pair: Option<Pair>,
    deadline: Option<Instant>,
    expiry: Option<i64>,
    wall: Option<i64>,
    pool: Option<[u8; 32]>,
}
pub(crate) struct Previous {
    handle: [u8; 32],
}
pub(crate) struct RequestOwner {
    lease: LeaseOwner,
    cell: Arc<Mutex<Cell>>,
    consumed: std::sync::atomic::AtomicBool,
}
impl RequestOwner {
    pub(crate) fn new(lease: LeaseOwner) -> Self {
        Self {
            lease,
            consumed: std::sync::atomic::AtomicBool::new(false),
            cell: Arc::new(Mutex::new(Cell {
                phase: Phase::Vacant,
                pair: None,
                deadline: None,
                expiry: None,
                wall: None,
                pool: None,
            })),
        }
    }
    fn clear(&self) {
        let mut c = match self.cell.lock() {
            Ok(c) => c,
            Err(e) => e.into_inner(),
        };
        c.pair.take();
        c.phase = Phase::Terminal;
        c.deadline = None;
        c.expiry = None;
        c.wall = None;
        c.pool = None;
    }
    fn stage(&self, pair: Pair) -> Result<(), Failure> {
        let _gate = self
            .lease
            .lease
            .checked_gate()
            .map_err(Failure::authority)?;
        let mut c = self.cell.lock().map_err(|_| Failure::unavailable())?;
        if !matches!(c.phase, Phase::Vacant) {
            return Err(Failure::invalid());
        }
        c.pair = Some(pair);
        c.phase = Phase::Pending;
        Ok(())
    }
    fn replace_pending(&self, pair: Pair) -> Result<(), Failure> {
        let _gate = self
            .lease
            .lease
            .checked_gate()
            .map_err(Failure::authority)?;
        let mut c = self.cell.lock().map_err(|_| Failure::unavailable())?;
        if !matches!(c.phase, Phase::Pending) {
            return Err(Failure::invalid());
        }
        c.pair = Some(pair);
        Ok(())
    }
    fn ready(
        &self,
        deadline: Instant,
        expiry: i64,
        wall: i64,
        pool: [u8; 32],
    ) -> Result<(), Failure> {
        let _gate = self
            .lease
            .lease
            .checked_gate()
            .map_err(Failure::authority)?;
        let mut c = self.cell.lock().map_err(|_| Failure::unavailable())?;
        // Gate->cell is unchanged. Check after cell acquisition as well, so
        // lock/maintenance elapsed time cannot publish an expired Ready value.
        if deadline <= Instant::now() {
            return Err(Failure::unavailable());
        }
        if !matches!(c.phase, Phase::Pending) {
            return Err(Failure::invalid());
        }
        c.deadline = Some(deadline);
        c.expiry = Some(expiry);
        c.wall = Some(wall);
        c.pool = Some(pool);
        c.phase = Phase::Ready;
        Ok(())
    }
    #[cfg(test)]
    fn is_ready(&self) -> bool {
        self.cell
            .lock()
            .is_ok_and(|c| matches!(c.phase, Phase::Ready))
    }
}
impl Drop for RequestOwner {
    fn drop(&mut self) {
        let mut gate = match self.lease.lease.gate.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        gate.active = false;
        self.clear();
    }
}
struct Pending<'a> {
    owner: &'a RequestOwner,
    armed: bool,
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.owner.clear();
        }
    }
}
struct Consumed {
    lease: Arc<Lease>,
    deadline: Instant,
}
impl Consumed {
    // Entirely private callback boundary; HTTP supplies only the adopted trusted
    // named verifier with a sanitized snapshot, never this factory to user code.
    async fn verify<F, Fut>(
        self,
        owner: &browser::OwnerBinding,
        verifier: F,
    ) -> Result<VerifiedIdentity, Failure>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<VerifiedIdentity, super::Failure>>,
    {
        owner.present()?;
        if !Arc::ptr_eq(&self.lease, &owner.lease) {
            return Err(Failure::denied());
        }
        {
            let _gate = self.lease.checked_gate().map_err(Failure::authority)?;
            if self.deadline <= Instant::now() {
                return Err(Failure::expired());
            }
        }
        let identity = verifier().await.map_err(Failure::verifier)?;
        owner.present()?;
        {
            let _gate = self.lease.checked_gate().map_err(Failure::authority)?;
            if self.deadline <= Instant::now() {
                return Err(Failure::expired());
            }
        }
        if !matches!(&identity.credential.source, CredentialSource::Bearer)
            || identity.expires_at <= Instant::now()
        {
            return Err(Failure::rejected_identity());
        }
        Ok(identity)
    }
}
#[cfg(test)]
struct PostcommitMaintenanceWait {
    arrived: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
#[cfg(test)]
impl PostcommitMaintenanceWait {
    fn new() -> Self {
        Self {
            arrived: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        }
    }
}
// Validated UTC ceil to the initial, earlier monotonic instant. This never
// reissues TTL after later SQL/maintenance waits; equality/unknown time refuses.
fn bootstrap_deadline(clock: &session::PreloginClock, expiry: i64) -> Result<Instant, Failure> {
    let remaining = expiry
        .checked_sub(clock.ceil_ms)
        .ok_or_else(Failure::unavailable)?;
    let remaining = u64::try_from(remaining).map_err(|_| Failure::unavailable())?;
    if remaining == 0 {
        return Err(Failure::unavailable());
    }
    clock
        .monotonic
        .checked_add(Duration::from_millis(remaining))
        .ok_or_else(Failure::unavailable)
}
impl Pool {
    async fn check(&self, tx: &sqlite::Tx, wall: &mut Option<i64>) -> Result<Metadata, Failure> {
        let context_clock = self
            .context
            .verify(tx, wall)
            .await
            .map_err(Failure::session)?;
        let schema=sqlite::all::<Schema>(tx,sqlite::literal("SELECT name,sql FROM sqlite_schema WHERE (name GLOB '__nagi_prelogin_*' OR tbl_name GLOB '__nagi_prelogin_*') AND name NOT GLOB 'sqlite_autoindex_*' ORDER BY name"),sqlite::parameters()).await.map_err(Failure::database)?;
        let expected = [
            ("__nagi_prelogin_expiry", INDEX),
            ("__nagi_prelogin_meta", META),
            ("__nagi_prelogin_ready", ROWS),
        ];
        if schema.len() != expected.len()
            || schema
                .iter()
                .zip(expected)
                .any(|(r, (n, q))| r.name != n || r.sql.as_deref() != Some(q))
        {
            return Err(Failure::unavailable());
        }
        let m = metadata(tx).await?;
        if m.version != 1
            || m.incarnation.as_slice() != self.incarnation
            || !self.context.matches_incarnation(&m.session_incarnation)
            || m.config != self.config
            || m.window_start < 0
            || m.window_start > m.last_wall
            || m.issued < 0
            || m.issued > self.options.issuance_per_window
        {
            return Err(Failure::unavailable());
        }
        session::PreloginContext::track_sample(wall, context_clock, m.last_wall)
            .map_err(Failure::session)?;
        Ok(m)
    }
    fn clock(&self, wall: &mut Option<i64>) -> Result<session::PreloginClock, Failure> {
        session::PreloginContext::track_sample(
            wall,
            self.context.sample().map_err(Failure::session)?,
            0,
        )
        .map_err(Failure::session)
    }
    async fn maintenance(&self, tx: &sqlite::Tx, wall: i64) -> Result<(), Failure> {
        let p = sqlite::bind_i64(sqlite::parameters(), wall);
        let p = sqlite::bind_i64(p, self.options.cleanup_batch);
        sqlite::exec(tx,sqlite::literal("DELETE FROM __nagi_prelogin_ready WHERE handle IN (SELECT handle FROM __nagi_prelogin_ready WHERE expiry_ms<=? ORDER BY expiry_ms,handle LIMIT ?)"),p).await.map_err(Failure::database)?;
        sqlite::exec(
            tx,
            sqlite::literal(
                "UPDATE __nagi_prelogin_meta SET last_wall=max(last_wall,?) WHERE id=1",
            ),
            sqlite::bind_i64(sqlite::parameters(), wall),
        )
        .await
        .map_err(Failure::database)?;
        Ok(())
    }
    #[cfg(test)]
    async fn restoration_maintenance(&self, tx: &sqlite::Tx, wall: i64) -> Result<(), Failure> {
        if let Some(wait) = &self.postcommit_wait {
            // The IR02 fixture owns a queue of exactly2. Hold both actual
            // permits and poll the unchanged production maintenance future:
            // its first real SQL send must wait. No lock/gate is held here.
            let first = tx.reserve_exec().await.map_err(Failure::database)?;
            let second = tx.reserve_exec().await.map_err(Failure::database)?;
            let mut operation = Box::pin(self.maintenance(tx, wall));
            let pending = std::future::poll_fn(|cx| {
                std::task::Poll::Ready(operation.as_mut().poll(cx).is_pending())
            })
            .await;
            assert!(pending, "actual maintenance did not wait for queue permits");
            wait.arrived.notify_one();
            wait.release.notified().await;
            drop(first);
            drop(second);
            return operation.await;
        }
        self.maintenance(tx, wall).await
    }
    async fn restore(&self, wall: &mut Option<i64>) -> Result<(), Failure> {
        let tx = browser_observe!(
            RestoreAcquire,
            sqlite::begin(&self.native, sqlite::BeginMode::Immediate)
        )
        .map_err(Failure::database)?;
        let result = async {
            browser_observe!(RestoreCheck, self.check(&tx, wall))?;
            self.clock(wall)?;
            let observed = wall.ok_or_else(Failure::unavailable)?;
            #[cfg(test)]
            {
                browser_observe!(
                    RestoreMaintenance,
                    self.restoration_maintenance(&tx, observed)
                )
            }
            #[cfg(not(test))]
            {
                browser_observe!(RestoreMaintenance, self.maintenance(&tx, observed))
            }
        }
        .await;
        match result {
            Ok(()) => {
                browser_observe!(RestoreCommit, sqlite::commit(tx)).map_err(Failure::database)?;
                #[cfg(nagi_browser_login_diag)]
                super::browser_login_observer::restore_commit_ack();
                Ok(())
            }
            Err(mut e) => {
                browser_observe!(RestoreRollback, sqlite::rollback(tx))
                    .map_err(Failure::database)?;
                e.outcome = sqlite::Outcome::RolledBack;
                Err(e)
            }
        }
    }
    // One maintenance-only Tx retains postcommit observations before Ready or
    // Consumed. An Err still remembers known business commit independently.
    async fn postcommit(&self, wall: &mut Option<i64>) -> Result<(), Failure> {
        let sampled = self.clock(wall);
        self.restore(wall).await.map_err(Failure::after_commit)?;
        sampled.map(|_| ()).map_err(Failure::after_commit)
    }
    async fn finish<T>(
        &self,
        tx: sqlite::Tx,
        result: Result<T, Failure>,
        wall: &mut Option<i64>,
    ) -> Result<T, Failure> {
        match result {
            Ok(v) => {
                browser_observe!(BusinessCommit, sqlite::commit(tx)).map_err(|error| {
                    let e = Failure::database(error);
                    if e.outcome == sqlite::Outcome::Committed {
                        e.after_commit()
                    } else {
                        e
                    }
                })?;
                Ok(v)
            }
            Err(mut e) => {
                browser_observe!(BusinessRollback, sqlite::rollback(tx))
                    .map_err(Failure::database)?;
                #[cfg(nagi_browser_login_diag)]
                super::browser_login_observer::rollback_ack();
                e.outcome = sqlite::Outcome::RolledBack;
                if wall.is_some() {
                    self.restore(wall).await?;
                }
                Err(e)
            }
        }
    }
    async fn bootstrap(&self, owner: &RequestOwner, old: Option<Previous>) -> Result<(), Failure> {
        #[cfg(all(test, nagi_auth_stage_diag))]
        let mut trace = Trace::new(&self.stage_slot, Op::Bootstrap);
        #[cfg(all(test, nagi_auth_stage_diag))]
        trace.record(Stage::Entry, Event::Seen, None, None);
        // Async laziness means an unpolled bootstrap performs no entropy/SQL.
        owner.lease.lease.validate().map_err(Failure::authority)?;
        #[cfg(test)]
        self.entropy_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let (pair, mut input) = fresh_pair()?;
        owner.stage(pair)?;
        let mut pending = Pending { owner, armed: true };
        let tx = sqlite::begin(&self.native, sqlite::BeginMode::Immediate)
            .await
            .map_err(Failure::database)?;
        #[cfg(all(test, nagi_auth_stage_diag))]
        trace.record(Stage::BeginAck, Event::Passed, None, None);
        let mut wall = None;
        let result = async {
            let m=self.check(&tx,&mut wall).await?;
            let c=self.clock(&mut wall)?;

            self.maintenance(&tx,c.ceil_ms).await?;
            let start=if c.floor_ms.checked_sub(m.window_start).is_some_and(|d|d>=self.options.window_ms){
                c.floor_ms}
            else{
                m.window_start}
            ;
            let issued=if start==m.window_start{
                m.issued}
            else{
                0}
            ;
            if issued>=self.options.issuance_per_window{
                return Err(Failure::unavailable());
            }
            if let Some(old)=old{
                let reserve=tx.reserve_exec().await.map_err(Failure::database)?;
                self.clock(&mut wall)?;
                let reply={
                    let _gate=owner.lease.lease.checked_gate().map_err(Failure::authority)?;
                    let reply=reserve.enqueue(sqlite::literal("DELETE FROM __nagi_prelogin_ready WHERE handle=?"),sqlite::bind_bytes(sqlite::parameters(),old.handle.to_vec()));
                    #[cfg(test)] self.business_enqueues.fetch_add(1,std::sync::atomic::Ordering::SeqCst);
                    reply}
                ;
                reply.await.map_err(Failure::database)?;
            }
            let stored=sqlite::query::<Count>(&tx,sqlite::literal("SELECT count(*) AS n FROM __nagi_prelogin_ready"),sqlite::parameters()).await.map_err(Failure::database)?.ok_or_else(Failure::unavailable)?.0;
            let live=sqlite::query::<Count>(&tx,sqlite::literal("SELECT count(*) AS n FROM __nagi_prelogin_ready WHERE expiry_ms>?"),sqlite::bind_i64(sqlite::parameters(),c.ceil_ms)).await.map_err(Failure::database)?.ok_or_else(Failure::unavailable)?.0;
            if stored>=self.options.max_stored||live>=self.options.max_live||stored.checked_add(1).and_then(|n|n.checked_mul(ROW_BYTES)).is_none_or(|v|v>self.options.max_payload_bytes){
                return Err(Failure::unavailable());
            }
            let expiry=c.floor_ms.checked_add(self.options.ttl_ms).ok_or_else(Failure::unavailable)?;
            let deadline=bootstrap_deadline(&c,expiry)?;
            #[cfg(all(test, nagi_auth_stage_diag))]
            trace.record(Stage::Expiry, Event::Nominal, Some((expiry,c.ceil_ms)), Some((deadline,c.monotonic)));
            let mut inserted=false;

            for attempt in 0..self.options.collision_attempts {
                if attempt>0 {
                    let (pair,next)=fresh_pair()?;
                    owner.replace_pending(pair)?;
                    input=next;
                }
                let reserve=tx.reserve_exec().await.map_err(Failure::database)?;
                // No second queue send while holding this permit (queue may be 1).
                // The metadata/source was read in this same Immediate Tx; other
                // writers cannot alter it. Only the clock is sampled after wait.
                let fresh=self.clock(&mut wall)?;

                if fresh.ceil_ms>=expiry{
                    #[cfg(all(test, nagi_auth_stage_diag))]
                    trace.record(Stage::Precommit, Event::Refused, Some((expiry,fresh.ceil_ms)), Some((deadline,fresh.monotonic)));
                    return Err(Failure::unavailable());
                }
                #[cfg(all(test, nagi_auth_stage_diag))]
                trace.record(Stage::Precommit, Event::Passed, Some((expiry,fresh.ceil_ms)), Some((deadline,fresh.monotonic)));
                let p=sqlite::bind_bytes(sqlite::parameters(),input.handle.to_vec());
                let p=sqlite::bind_bytes(p,input.token.to_vec());
                let p=sqlite::bind_i64(p,expiry);
                let reply={
                    let _gate=owner.lease.lease.checked_gate().map_err(Failure::authority)?;
                    let reply=reserve.enqueue(sqlite::literal("INSERT OR IGNORE INTO __nagi_prelogin_ready(handle,token,expiry_ms) VALUES(?,?,?)"),p);
                    #[cfg(test)] self.business_enqueues.fetch_add(1,std::sync::atomic::Ordering::SeqCst);
                    reply}
                ;
                let affected=reply.await.map_err(Failure::database)?;
                if affected==1 {
                    inserted=true;
                    break;
                }
                if affected!=0 {
                    return Err(Failure::unavailable());
                }
            }
            if !inserted {
                return Err(Failure::unavailable());
            }
            let p=sqlite::bind_i64(sqlite::parameters(),start);
            let p=sqlite::bind_i64(p,issued.checked_add(1).ok_or_else(Failure::unavailable)?);
            let p=sqlite::bind_i64(p,wall.ok_or_else(Failure::unavailable)?);
            sqlite::exec(&tx,sqlite::literal("UPDATE __nagi_prelogin_meta SET window_start=?,issued=?,last_wall=max(last_wall,?) WHERE id=1"),p).await.map_err(Failure::database)?;
            let final_sample=self.clock(&mut wall)?;
            if expiry<=final_sample.ceil_ms{
                #[cfg(all(test, nagi_auth_stage_diag))]
                trace.record(Stage::Precommit, Event::Refused, Some((expiry,final_sample.ceil_ms)), Some((deadline,final_sample.monotonic)));
                return Err(Failure::unavailable());
            }
            #[cfg(all(test, nagi_auth_stage_diag))]
            trace.record(Stage::Precommit, Event::Passed, Some((expiry,final_sample.ceil_ms)), Some((deadline,final_sample.monotonic)));
            self.maintenance(&tx,wall.ok_or_else(Failure::unavailable)?).await?;
            Ok((expiry,deadline))
        }
        .await;
        let finished = self.finish(tx, result, &mut wall).await;
        #[cfg(all(test, nagi_auth_stage_diag))]
        {
            trace.record(
                Stage::FinishAck,
                if finished.is_ok() {
                    Event::Passed
                } else {
                    Event::Refused
                },
                None,
                None,
            );
            if let Err(e) = &finished {
                trace.returned(Some((e.kind, e.outcome)), None, None);
            }
        }
        let (expiry, deadline) = finished?;
        #[cfg(all(test, nagi_auth_stage_diag))]
        trace.record(
            Stage::PostcommitBefore,
            Event::Seen,
            wall.map(|w| (expiry, w)),
            None,
        );
        let maintenance = self.postcommit(&mut wall).await;
        #[cfg(all(test, nagi_auth_stage_diag))]
        {
            trace.record(
                Stage::PostcommitAck,
                if maintenance.is_ok() {
                    Event::Passed
                } else {
                    Event::Refused
                },
                wall.map(|w| (expiry, w)),
                None,
            );
            if let Err(e) = &maintenance {
                trace.returned(Some((e.kind, e.outcome)), wall.map(|w| (expiry, w)), None);
            }
        }
        maintenance?;
        if wall.is_some_and(|v| v >= expiry) {
            #[cfg(all(test, nagi_auth_stage_diag))]
            trace.record(
                Stage::Gate,
                Event::WallExpired,
                wall.map(|w| (expiry, w)),
                None,
            );
            #[cfg(all(test, nagi_auth_stage_diag))]
            trace.returned(
                Some((FailureKind::Unavailable, sqlite::Outcome::Committed)),
                wall.map(|w| (expiry, w)),
                None,
            );
            return Err(Failure::unavailable().after_commit());
        }
        owner
            .ready(
                deadline,
                expiry,
                wall.ok_or_else(|| Failure::unavailable().after_commit())?,
                self.incarnation,
            )
            .map_err(Failure::after_commit)?;
        pending.armed = false;
        #[cfg(all(test, nagi_auth_stage_diag))]
        trace.returned(None, wall.map(|w| (expiry, w)), None);
        Ok(())
    }
    async fn claim(&self, owner: &RequestOwner, input: ClaimInput) -> Result<Consumed, Failure> {
        #[cfg(all(test, nagi_auth_stage_diag))]
        let mut trace = Trace::new(&self.stage_slot, Op::Claim);
        #[cfg(all(test, nagi_auth_stage_diag))]
        trace.record(Stage::Entry, Event::Seen, None, None);
        owner.lease.lease.validate().map_err(Failure::authority)?;
        let tx = browser_observe!(
            ClaimBegin,
            sqlite::begin(&self.native, sqlite::BeginMode::Immediate)
        )
        .map_err(Failure::database)?;
        #[cfg(all(test, nagi_auth_stage_diag))]
        trace.record(Stage::BeginAck, Event::Passed, None, None);
        let mut wall = None;
        let result = async {
            browser_observe!(ClaimCheck, self.check(&tx, &mut wall))?;
            let c = self.clock(&mut wall)?;

            let row =
                browser_observe!(ClaimRow, sqlite::query::<Row>(
                &tx,
                sqlite::literal(
                    "SELECT handle,token,expiry_ms FROM __nagi_prelogin_ready WHERE handle=?",
                ),
                sqlite::bind_bytes(sqlite::parameters(), input.handle.to_vec()),
            ))
                .map_err(Failure::database)?;
            #[cfg(nagi_browser_login_diag)]
            if row.is_none() {
                super::browser_login_observer::row_missing();
            }
            let row = row.ok_or_else(Failure::denied)?;
            let handle: [u8; 32] = row.handle.try_into().map_err(|_| Failure::unavailable())?;
            let token: [u8; 32] = row.token.try_into().map_err(|_| Failure::unavailable())?;
            if !bool::from(handle.ct_eq(&input.handle) & token.ct_eq(&input.token))
                || c.ceil_ms >= row.expiry
            {
                #[cfg(all(test, nagi_auth_stage_diag))]
                trace.record(Stage::Row, Event::Miss, Some((row.expiry, c.ceil_ms)), None);
                return Err(Failure::denied());
            }
            #[cfg(all(test, nagi_auth_stage_diag))]
            trace.record(
                Stage::Row,
                Event::Match,
                Some((row.expiry, c.ceil_ms)),
                None,
            );
            let remaining = u64::try_from(
                row.expiry
                    .checked_sub(c.ceil_ms)
                    .ok_or_else(Failure::denied)?,
            )
            .map_err(|_| Failure::denied())?;
            let deadline = c
                .monotonic
                .checked_add(Duration::from_millis(remaining))
                .ok_or_else(Failure::unavailable)?;
            #[cfg(all(test, nagi_auth_stage_diag))]
            trace.record(
                Stage::Expiry,
                Event::Nominal,
                Some((row.expiry, c.ceil_ms)),
                Some((deadline, c.monotonic)),
            );
            {
                let mut gate = owner
                    .lease
                    .lease
                    .checked_gate()
                    .map_err(Failure::authority)?;
                gate.expires_at = gate.expires_at.min(deadline);
            }
            #[cfg(test)]
            if let Some(wait) = &self.preconsume_wait {
                // Exercise the actual bounded reserve queue after gate
                // shortening, before any business DELETE is admitted.
                let first = tx.reserve_exec().await.map_err(Failure::database)?;
                let second = tx.reserve_exec().await.map_err(Failure::database)?;
                let mut reservation = std::pin::pin!(tx.reserve_exec());
                let pending = std::future::poll_fn(|cx| {
                    std::task::Poll::Ready(reservation.as_mut().poll(cx).is_pending())
                })
                .await;
                assert!(
                    pending,
                    "actual claim reserve did not wait for queue permits"
                );
                wait.arrived.notify_one();
                wait.release.notified().await;
                drop(first);
                drop(second);
                drop(reservation.await.map_err(Failure::database)?);
            }
            let reserve =
                browser_observe!(BusinessReserve, tx.reserve_exec()).map_err(Failure::database)?;
            let fresh = self.clock(&mut wall)?;

            if fresh.ceil_ms >= row.expiry || deadline <= Instant::now() {
                #[cfg(all(test, nagi_auth_stage_diag))]
                trace.record(
                    Stage::Precommit,
                    Event::Refused,
                    Some((row.expiry, fresh.ceil_ms)),
                    Some((deadline, fresh.monotonic)),
                );
                return Err(Failure::denied());
            }
            #[cfg(all(test, nagi_auth_stage_diag))]
            trace.record(
                Stage::Precommit,
                Event::Passed,
                Some((row.expiry, fresh.ceil_ms)),
                Some((deadline, fresh.monotonic)),
            );
            let p = sqlite::bind_bytes(sqlite::parameters(), input.handle.to_vec());
            let p = sqlite::bind_i64(p, fresh.ceil_ms);
            let reply = {
                let _gate = owner
                    .lease
                    .lease
                    .checked_gate()
                    .map_err(Failure::authority)?;
                let reply = reserve.enqueue(
                    sqlite::literal(
                        "DELETE FROM __nagi_prelogin_ready WHERE handle=? AND expiry_ms>?",
                    ),
                    p,
                );
                #[cfg(test)]
                self.business_enqueues
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                reply
            };
            if browser_observe!(BusinessDelete, reply).map_err(Failure::database)? != 1 {
                return Err(Failure::denied());
            }
            let final_sample = self.clock(&mut wall)?;
            if final_sample.ceil_ms >= row.expiry || deadline <= Instant::now() {
                #[cfg(all(test, nagi_auth_stage_diag))]
                trace.record(
                    Stage::Precommit,
                    Event::Refused,
                    Some((row.expiry, final_sample.ceil_ms)),
                    Some((deadline, final_sample.monotonic)),
                );
                return Err(Failure::denied());
            }
            #[cfg(all(test, nagi_auth_stage_diag))]
            trace.record(
                Stage::Precommit,
                Event::Passed,
                Some((row.expiry, final_sample.ceil_ms)),
                Some((deadline, final_sample.monotonic)),
            );
            browser_observe!(
                BusinessMaintenance,
                self.maintenance(&tx, wall.ok_or_else(Failure::unavailable)?)
            )?;
            Ok((deadline, row.expiry))
        }
        .await;
        let (deadline, expiry) = match self.finish(tx, result, &mut wall).await {
            Ok(value) => {
                owner.mark_consumed();
                #[cfg(all(test, nagi_auth_stage_diag))]
                trace.record(Stage::FinishAck, Event::Passed, None, None);
                value
            }
            Err(error) => {
                if error.business_committed {
                    owner.mark_consumed();
                }
                #[cfg(all(test, nagi_auth_stage_diag))]
                {
                    trace.record(Stage::FinishAck, Event::Refused, None, None);
                    trace.returned(Some((error.kind, error.outcome)), None, None);
                }
                return Err(error);
            }
        };
        #[cfg(all(test, nagi_auth_stage_diag))]
        trace.record(
            Stage::PostcommitBefore,
            Event::Seen,
            wall.map(|w| (expiry, w)),
            None,
        );
        let maintenance = self.postcommit(&mut wall).await;
        #[cfg(all(test, nagi_auth_stage_diag))]
        {
            trace.record(
                Stage::PostcommitAck,
                if maintenance.is_ok() {
                    Event::Passed
                } else {
                    Event::Refused
                },
                wall.map(|w| (expiry, w)),
                None,
            );
            if let Err(e) = &maintenance {
                trace.returned(Some((e.kind, e.outcome)), wall.map(|w| (expiry, w)), None);
            }
        }
        maintenance?;
        if wall.is_some_and(|v| v >= expiry) {
            #[cfg(all(test, nagi_auth_stage_diag))]
            {
                trace.record(
                    Stage::Gate,
                    Event::WallExpired,
                    wall.map(|w| (expiry, w)),
                    None,
                );
                trace.returned(
                    Some((FailureKind::Expired, sqlite::Outcome::Committed)),
                    wall.map(|w| (expiry, w)),
                    None,
                );
            }
            return Err(Failure::expired().after_commit());
        }
        {
            let gate_result = owner
                .lease
                .lease
                .checked_gate()
                .map_err(Failure::authority)
                .map_err(Failure::after_commit);
            #[cfg(all(test, nagi_auth_stage_diag))]
            if let Err(e) = &gate_result {
                trace.record(
                    Stage::Gate,
                    Event::GateExpiredOrEnded,
                    wall.map(|w| (expiry, w)),
                    None,
                );
                trace.returned(Some((e.kind, e.outcome)), wall.map(|w| (expiry, w)), None);
            }
            let _gate = gate_result?;
            if deadline <= Instant::now() {
                #[cfg(all(test, nagi_auth_stage_diag))]
                trace.mono_expired_local(wall.map(|w| (expiry, w)));
                return Err(Failure::expired().after_commit());
            }
        }
        #[cfg(all(test, nagi_auth_stage_diag))]
        trace.returned(None, wall.map(|w| (expiry, w)), None);
        Ok(Consumed {
            lease: Arc::clone(&owner.lease.lease),
            deadline,
        })
    }
}

// Closed config parser for the optional authenticated ledger startup budget.
pub(super) async fn declared_bytes(
    tx: &sqlite::Tx,
    context: &session::PreloginContext,
) -> Result<i64, Failure> {
    let rows=sqlite::all::<Schema>(tx,sqlite::literal("SELECT name,sql FROM sqlite_schema WHERE (name GLOB '__nagi_prelogin_*' OR tbl_name GLOB '__nagi_prelogin_*') AND name NOT GLOB 'sqlite_autoindex_*' ORDER BY name"),sqlite::parameters()).await.map_err(Failure::database)?;
    if rows.is_empty() {
        return Ok(0);
    }
    let expected = [
        ("__nagi_prelogin_expiry", INDEX),
        ("__nagi_prelogin_meta", META),
        ("__nagi_prelogin_ready", ROWS),
    ];
    if rows.len() != expected.len()
        || rows
            .iter()
            .zip(expected)
            .any(|(r, (n, q))| r.name != n || r.sql.as_deref() != Some(q))
    {
        return Err(Failure::unavailable());
    }
    let m = metadata(tx).await?;
    let mut parts = m.config.splitn(11, '|');
    let mut values = [0i64; 10];
    for n in &mut values {
        let text = parts.next().ok_or_else(Failure::unavailable)?;
        *n = text.parse::<i64>().map_err(|_| Failure::unavailable())?;
        if n.to_string() != text {
            return Err(Failure::unavailable());
        }
    }
    let o = options(
        values[0], values[1], values[2], values[3], values[4], values[5], values[6], values[7],
        values[8], values[9],
    )?;
    fn sized(input: &str) -> Result<(&str, &str), Failure> {
        let (size, rest) = input.split_once(':').ok_or_else(Failure::unavailable)?;
        let n = size.parse::<usize>().map_err(|_| Failure::unavailable())?;
        if n.to_string() != size {
            return Err(Failure::unavailable());
        }
        Ok((
            rest.get(..n).ok_or_else(Failure::unavailable)?,
            rest.get(n..).ok_or_else(Failure::unavailable)?,
        ))
    }
    let (name, rest) = sized(parts.next().ok_or_else(Failure::unavailable)?)?;
    let rest = rest.strip_prefix('|').ok_or_else(Failure::unavailable)?;
    let (same, rest) = rest.split_once('|').ok_or_else(Failure::unavailable)?;
    let same = match same {
        "Strict" => session::SameSite::Strict,
        "Lax" => session::SameSite::Lax,
        _ => return Err(Failure::unavailable()),
    };
    cookie_options(name, same)?;
    let (origin, rest) = sized(rest)?;
    if !rest.is_empty()
        || !crate::http_server::BoundOrigin::is_canonical(origin)
        || m.version != 1
        || m.incarnation.len() != 32
        || !context.matches_incarnation(&m.session_incarnation)
        || m.last_wall < 0
        || m.window_start < 0
        || m.window_start > m.last_wall
        || m.issued < 0
        || m.issued > o.issuance_per_window
    {
        return Err(Failure::unavailable());
    }
    o.max_stored
        .checked_mul(ROW_BYTES)
        .ok_or_else(Failure::unavailable)
}
