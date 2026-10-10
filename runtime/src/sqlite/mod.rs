//! SQLite Pool/affine Tx: typed SQL sessions and observed native cleanup.
#[cfg(test)]
mod acquire_tests;
mod adapter;
#[cfg(test)]
mod adapter_tests;
#[cfg(test)]
mod comparison;
#[cfg(test)]
mod public_tests;
#[cfg(test)]
mod security_sf05_tests;
mod session;
#[cfg(test)]
mod tests;
use crate::{Error, ErrorKind, FromRow};
use rusqlite::types::Value;
use session::Sql;
#[cfg(test)]
use session::*;
pub use session::{ExecReservation, Tx};
use std::{fmt, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeginMode {
    Deferred,
    Immediate,
    Exclusive,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    Invalid,
    Closed,
    AcquireTimeout,
    Busy,
    Sql,
    Bind,
    Decode,
    Aborted,
    Cleanup,
    Worker,
    ReplyLost,
    CloseTimeout,
    Allocation,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    NotApplicable,
    Active,
    Committed,
    RolledBack,
    Unknown,
}

pub struct Failure {
    pub kind: FailureKind,
    pub outcome: Outcome,
    pub retired: bool,
    pub message: String,
    pub(super) primary: Option<Error>,
    pub(super) cleanup: Option<Error>,
}
impl Failure {
    fn duplicate(&self) -> Self {
        Self {
            kind: self.kind,
            outcome: self.outcome,
            retired: self.retired,
            message: self.message.clone(),
            primary: self.primary.clone(),
            cleanup: self.cleanup.clone(),
        }
    }
}
impl fmt::Debug for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Failure")
            .field("kind", &self.kind)
            .field("outcome", &self.outcome)
            .field("retired", &self.retired)
            .finish()
    }
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Failure {}
impl Failure {
    fn primary(kind: FailureKind, outcome: Outcome, message: impl Into<String>) -> Self {
        let message = message.into();
        let error_kind = match kind {
            FailureKind::Invalid | FailureKind::Bind => ErrorKind::Invalid,
            FailureKind::Busy => ErrorKind::Busy,
            FailureKind::Sql | FailureKind::Decode => ErrorKind::Database,
            _ => ErrorKind::Internal,
        };
        Self::primary_error(
            kind,
            outcome,
            Error {
                kind: error_kind,
                message,
            },
        )
    }
    fn primary_error(kind: FailureKind, outcome: Outcome, error: Error) -> Self {
        let kind = if kind == FailureKind::Sql && matches!(error.kind, ErrorKind::Busy) {
            FailureKind::Busy
        } else {
            kind
        };
        Self {
            kind,
            outcome,
            retired: false,
            message: error.message.clone(),
            primary: Some(error),
            cleanup: None,
        }
    }
    fn cleanup(outcome: Outcome, message: impl Into<String>) -> Self {
        Self::cleanup_error(outcome, Error::internal(message))
    }
    fn cleanup_error(outcome: Outcome, error: Error) -> Self {
        Self {
            kind: FailureKind::Cleanup,
            outcome,
            retired: true,
            message: error.message.clone(),
            primary: None,
            cleanup: Some(error),
        }
    }
    fn allocation(message: impl Into<String>) -> Self {
        Self::primary(FailureKind::Allocation, Outcome::NotApplicable, message)
    }
}

#[derive(Debug)]
pub struct Options {
    connections: usize,
    queue_capacity: usize,
    acquire: Duration,
    busy: Duration,
}
pub fn options(
    connections: i64,
    queue_capacity: i64,
    acquire_ms: i64,
    busy_ms: i64,
) -> Result<Options, Error> {
    fn capacity(value: i64) -> Result<usize, Error> {
        let value = usize::try_from(value)
            .map_err(|_| Error::invalid("capacity must be positive and fit usize"))?;
        if value == 0 || value > tokio::sync::Semaphore::MAX_PERMITS {
            return Err(Error::invalid("capacity exceeds Tokio permit range"));
        }
        Ok(value)
    }
    let connections = capacity(connections)?;
    let queue_capacity = capacity(queue_capacity)?;
    let acquire = checked_duration(acquire_ms)?;
    let busy = checked_duration(busy_ms)?;
    i32::try_from(busy_ms)
        .map_err(|_| Error::invalid("busy_ms exceeds SQLite i32 milliseconds range"))?;
    Ok(Options {
        connections,
        queue_capacity,
        acquire,
        busy,
    })
}
fn checked_duration(ms: i64) -> Result<Duration, Error> {
    let ms = u64::try_from(ms).map_err(|_| Error::invalid("milliseconds must be nonnegative"))?;
    let value = Duration::from_millis(ms);
    if std::time::Instant::now().checked_add(value).is_none() {
        return Err(Error::invalid("milliseconds exceed native deadline range"));
    }
    Ok(value)
}

pub struct Pool {
    adapter: adapter::Adapter,
    acquire: Duration,
}
impl fmt::Debug for Pool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pool")
            .field("closing", &self.adapter.is_closing())
            .finish()
    }
}
/// Own the path before returning the future. Native connections are lazy.
pub fn open(
    path: &str,
    options: Options,
) -> impl std::future::Future<Output = Result<Pool, Failure>> + Send + 'static {
    let path = path.to_owned();
    async move {
        if path.is_empty()
            || path.starts_with("file:")
            || (path == ":memory:" && options.connections != 1)
        {
            return Err(Failure::primary(
                FailureKind::Invalid,
                Outcome::NotApplicable,
                "unsupported SQLite path/options combination",
            ));
        }
        let config = session::Config {
            path: Some(path.into()),
            queue_capacity: Some(options.queue_capacity),
            busy: options.busy,
            ..Default::default()
        };
        Ok(Pool {
            adapter: adapter::Adapter::with_capacity(
                config,
                Default::default(),
                options.connections,
            ),
            acquire: options.acquire,
        })
    }
}
pub fn clone_pool(pool: &Pool) -> Pool {
    Pool {
        adapter: pool.adapter.clone_handle(),
        acquire: pool.acquire,
    }
}
pub async fn begin(pool: &Pool, mode: BeginMode) -> Result<Tx, Failure> {
    let budget = adapter::AcquireBudget::after_at(tokio::time::Instant::now(), pool.acquire)
        .ok_or_else(|| {
            Failure::primary(
                FailureKind::Invalid,
                Outcome::NotApplicable,
                "acquire deadline overflow",
            )
        })?;
    pool.adapter.begin_mode_with_budget(mode, budget).await
}
pub struct Parameters(Vec<Value>);
pub fn parameters() -> Parameters {
    Parameters(Vec::new())
}
pub fn bind_i64(mut parameters: Parameters, value: i64) -> Parameters {
    parameters.0.push(Value::Integer(value));
    parameters
}
pub fn bind_f64(mut parameters: Parameters, value: f64) -> Result<Parameters, Error> {
    if !value.is_finite() {
        return Err(Error::invalid("finite SQLite float required"));
    }
    parameters.0.push(Value::Real(value));
    Ok(parameters)
}
pub fn bind_text(mut parameters: Parameters, value: String) -> Parameters {
    parameters.0.push(Value::Text(value));
    parameters
}
pub fn bind_bytes(mut parameters: Parameters, value: Vec<u8>) -> Parameters {
    parameters.0.push(Value::Blob(value));
    parameters
}
pub fn bind_null(mut parameters: Parameters) -> Parameters {
    parameters.0.push(Value::Null);
    parameters
}
/// Fixed SQL structure. Values belong in Parameters. Construction from a
/// dynamic owned string and public field access are deliberately unavailable.
/// Trusted Rust adapters must use reviewed static statements.
/// ```compile_fail
/// let dynamic = String::from("SELECT 1");
/// nagi_runtime::sqlite::literal(&dynamic);
/// ```
#[derive(Clone, Copy)]
pub struct Query(&'static str);
impl fmt::Debug for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Query").finish_non_exhaustive()
    }
}
/// Nagi's canonical checker requires a direct string literal at this operation.
/// Rust host callers are a trusted boundary; SQL semantics/authorization remain
/// checked by SQLite and the reviewed adapter's predicates, respectively.
pub fn literal(sql: &'static str) -> Query {
    Query(sql)
}
pub async fn query<T: FromRow>(
    tx: &Tx,
    query: Query,
    parameters: Parameters,
) -> Result<Option<T>, Failure> {
    tx.query_sql(Sql::Static(query.0), parameters.0).await
}
pub async fn all<T: FromRow>(
    tx: &Tx,
    query: Query,
    parameters: Parameters,
) -> Result<Vec<T>, Failure> {
    tx.all_sql(Sql::Static(query.0), parameters.0).await
}
pub async fn exec(tx: &Tx, query: Query, parameters: Parameters) -> Result<i64, Failure> {
    tx.exec_sql(Sql::Static(query.0), parameters.0).await
}
pub async fn commit(tx: Tx) -> Result<(), Failure> {
    tx.finish(session::Finish::Commit).await
}
pub async fn rollback(tx: Tx) -> Result<(), Failure> {
    tx.finish(session::Finish::Rollback).await
}
pub async fn close(pool: &Pool, timeout_ms: i64) -> Result<(), Failure> {
    let timeout = checked_duration(timeout_ms).map_err(|error| {
        Failure::primary_error(FailureKind::Invalid, Outcome::NotApplicable, error)
    })?;
    pool.adapter.close(timeout).await
}
pub fn copy_primary_error(problem: &Failure) -> Option<Error> {
    problem.primary.clone()
}
pub fn copy_cleanup_error(problem: &Failure) -> Option<Error> {
    problem.cleanup.clone()
}
