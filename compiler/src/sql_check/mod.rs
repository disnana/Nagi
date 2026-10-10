//! Explicit offline SQL checks. Nothing here is called by ordinary check or
//! generated applications. The SQLite engine runs in a bounded child process.
#[cfg(feature = "sql-check")]
mod collect;
#[cfg(feature = "sql-check")]
mod engine;

#[cfg(feature = "sql-check")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "sql-check")]
use std::{
    fs,
    io::{Read, Write},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[cfg(feature = "sql-check")]
pub(super) const MAX_SCHEMA_BYTES: usize = 2 * 1024 * 1024;
#[cfg(feature = "sql-check")]
pub(super) const MAX_SQL_BYTES: usize = 256 * 1024;
#[cfg(feature = "sql-check")]
pub(super) const MAX_QUERIES: usize = 4096;
#[cfg(feature = "sql-check")]
pub(super) const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
#[cfg(feature = "sql-check")]
const TIMEOUT: Duration = Duration::from_secs(5);
const WORKER_ARGUMENT: &str = "--nagi-sql-check-worker";

#[cfg(feature = "sql-check")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Operation {
    SqliteRows,
    SqliteExec,
}

#[cfg(feature = "sql-check")]
pub(super) struct Site {
    pub line: usize,
    pub operation: Option<Operation>,
    pub operation_name: String,
    pub sql: Option<String>,
    pub bind_count: Option<usize>,
    pub fields: Vec<String>,
    pub reason: Option<String>,
}

#[cfg(feature = "sql-check")]
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Query {
    pub operation: Operation,
    pub sql: String,
    pub bind_count: Option<usize>,
    pub fields: Vec<String>,
}

#[cfg(feature = "sql-check")]
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub schema: String,
    pub queries: Vec<Query>,
}

#[cfg(feature = "sql-check")]
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QueryOutcome {
    pub index: usize,
    pub error: Option<String>,
}

#[cfg(feature = "sql-check")]
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Response {
    pub schema_error: Option<String>,
    pub queries: Vec<QueryOutcome>,
}

/// This internal protocol cannot reach project parsing, Cargo, or Rust adapters.
pub(crate) fn worker(args: &[String]) -> Result<bool, String> {
    if args.first().map(String::as_str) != Some(WORKER_ARGUMENT) {
        return Ok(false);
    }
    if args.len() != 1 {
        return Err("SQL worker takes no arguments".into());
    }
    #[cfg(feature = "sql-check")]
    {
        let bytes = read_bounded(std::io::stdin().lock(), MAX_MESSAGE_BYTES)?;
        let request = serde_json::from_slice(&bytes)
            .map_err(|error| format!("Invalid SQL worker request: {error}"))?;
        let response = engine::validate(request);
        let bytes = encode_bounded(&response)?;
        std::io::stdout()
            .lock()
            .write_all(&bytes)
            .map_err(|error| format!("Cannot write SQL worker response: {error}"))?;
        Ok(true)
    }
    #[cfg(not(feature = "sql-check"))]
    Err("This compiler was built without the sql-check feature".into())
}

#[cfg(feature = "sql-check")]
fn read_bounded(reader: impl Read, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot read SQL check input/output: {error}"))?;
    if bytes.len() > limit {
        return Err(format!(
            "SQL check input/output exceeds the {limit} byte limit"
        ));
    }
    Ok(bytes)
}

#[cfg(feature = "sql-check")]
struct Worker(Child);

#[cfg(feature = "sql-check")]
impl Worker {
    fn terminate(&mut self) -> Result<std::process::ExitStatus, String> {
        if let Err(error) = self.0.kill() {
            if self.0.try_wait().map_err(|e| e.to_string())?.is_none() {
                return Err(format!("Cannot terminate SQL worker: {error}"));
            }
        }
        self.0
            .wait()
            .map_err(|error| format!("Cannot reap SQL worker: {error}"))
    }
}

#[cfg(feature = "sql-check")]
fn encode_bounded(value: &impl Serialize) -> Result<Vec<u8>, String> {
    struct Buffer(Vec<u8>);
    impl Write for Buffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.0.len().saturating_add(bytes.len()) > MAX_MESSAGE_BYTES {
                return Err(std::io::Error::other(
                    "SQL check exceeds the 8 MiB encoded input/output limit",
                ));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut buffer = Buffer(Vec::new());
    serde_json::to_writer(&mut buffer, value)
        .map_err(|error| format!("Cannot encode SQL check input/output: {error}"))?;
    Ok(buffer.0)
}

#[cfg(feature = "sql-check")]
impl Drop for Worker {
    fn drop(&mut self) {
        // Also reap on every error path. Killing a child which has already
        // exited is harmless; wait retains its completed status.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(feature = "sql-check")]
fn run_worker(
    mut command: Command,
    request: Vec<u8>,
    timeout: Duration,
) -> Result<Response, String> {
    let deadline = Instant::now() + timeout;
    let mut child = Worker(
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("Cannot start SQL check worker: {error}"))?,
    );
    let mut stdin = child.0.stdin.take().expect("piped worker stdin");
    let stdout = child.0.stdout.take().expect("piped worker stdout");
    let stderr = child.0.stderr.take().expect("piped worker stderr");
    // Concurrent pipes avoid deadlock on either a large request or diagnostics.
    // The deadline covers pipe IO as well as SQLite preparation.
    let writer = thread::spawn(move || stdin.write_all(&request));
    let reader = thread::spawn(move || read_bounded(stdout, MAX_MESSAGE_BYTES));
    let errors = thread::spawn(move || read_bounded(stderr, MAX_MESSAGE_BYTES));
    let status = loop {
        match child.0.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                break match child.terminate() {
                    Ok(status) => Err(format!("SQL check exceeded its worker deadline (5 seconds); worker terminated and reaped ({status})")),
                    Err(error) => Err(format!("SQL check exceeded its worker deadline; {error}")),
                };
            }
            Err(error) => {
                let _ = child.0.kill();
                let _ = child.0.wait();
                break Err(format!("Cannot wait for SQL check worker: {error}"));
            }
        }
    };
    let written = writer
        .join()
        .map_err(|_| "SQL worker input thread failed")?;
    let output = reader
        .join()
        .map_err(|_| "SQL worker output thread failed")?;
    let error_output = errors
        .join()
        .map_err(|_| "SQL worker diagnostics thread failed")?;
    let status = status?;
    if !status.success() {
        let errors = error_output?;
        return Err(format!(
            "SQL check worker failed ({status}): {}",
            String::from_utf8_lossy(&errors).trim()
        ));
    }
    written.map_err(|error| format!("Cannot send SQL check request: {error}"))?;
    let output = output?;
    serde_json::from_slice(&output)
        .map_err(|error| format!("Invalid SQL check worker response: {error}"))
}

pub(crate) fn check(
    program: &crate::ast::Program,
    sources: &crate::source::Sources,
    schema: &std::path::Path,
) -> Result<(), String> {
    #[cfg(not(feature = "sql-check"))]
    {
        let _ = (program, sources, schema);
        Err("This compiler was built without the sql-check feature".into())
    }
    #[cfg(feature = "sql-check")]
    {
        let schema_error =
            |message: String| format!("error: SQL schema: {message}\n --> {}", schema.display());
        if !fs::metadata(schema)
            .map_err(|e| schema_error(e.to_string()))?
            .is_file()
        {
            return Err(schema_error("expected a regular DDL snapshot file".into()));
        }
        let file = fs::File::open(schema).map_err(|error| schema_error(error.to_string()))?;
        if !file
            .metadata()
            .map_err(|e| schema_error(e.to_string()))?
            .is_file()
        {
            return Err(schema_error("expected a regular DDL snapshot file".into()));
        }
        let schema_text = read_bounded(file, MAX_SCHEMA_BYTES).map_err(schema_error)?;
        let schema_text = String::from_utf8(schema_text)
            .map_err(|error| schema_error(format!("expected UTF-8: {error}")))?;
        let sites = collect::collect(program).map_err(|error| sources.diagnostic(&error))?;
        if sites.len() > MAX_QUERIES {
            return Err(format!(
                "SQL check exceeds the {MAX_QUERIES} call-site limit"
            ));
        }
        let mut queries = Vec::new();
        let mut selected = Vec::new();
        let mut size = schema_text.len();
        for site in &sites {
            if let (Some(operation), Some(sql)) = (site.operation, &site.sql) {
                if sql.len() > MAX_SQL_BYTES {
                    return Err(sources.diagnostic(&format!(
                        "line {}: SQL query exceeds the 256 KiB limit",
                        site.line
                    )));
                }
                size = size.saturating_add(sql.len());
                if size > MAX_MESSAGE_BYTES {
                    return Err("SQL check exceeds the 8 MiB total input limit".into());
                }
                queries.push(Query {
                    operation,
                    sql: sql.clone(),
                    bind_count: site.bind_count,
                    fields: site.fields.clone(),
                });
                selected.push(site);
            }
        }
        let request = encode_bounded(&Request {
            schema: schema_text,
            queries,
        })?;
        let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
        command.arg(WORKER_ARGUMENT);
        let response = run_worker(command, request, TIMEOUT)?;
        if let Some(error) = response.schema_error {
            return Err(schema_error(error));
        }
        if response.queries.len() != selected.len()
            || response
                .queries
                .iter()
                .enumerate()
                .any(|(i, q)| q.index != i)
        {
            return Err("SQL check worker returned inconsistent query metadata".into());
        }
        let runtime = sites.len() - selected.len();
        eprintln!(
            "SQL checked {} literal queries; {runtime} runtime/unsupported sites",
            selected.len()
        );
        for site in &sites {
            if let Some(reason) = &site.reason {
                let location = sources
                    .location(site.line)
                    .map(|location| format!("{}:{}", location.path.display(), location.line))
                    .unwrap_or_else(|| format!("line {}", site.line));
                let category = if site.operation.is_some() && site.sql.is_some() {
                    "bind unchecked"
                } else {
                    "runtime/unsupported"
                };
                eprintln!(
                    "SQL {category}: {}: {reason}\n --> {location}",
                    site.operation_name
                );
            }
        }
        let failures: Vec<_> = response
            .queries
            .into_iter()
            .filter_map(|result| {
                result.error.map(|error| {
                    let site = selected[result.index];
                    sources.diagnostic(&format!(
                        "line {}: SQL {}: {error}",
                        site.line, site.operation_name
                    ))
                })
            })
            .collect();
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("\n"))
        }
    }
}

#[cfg(all(test, feature = "sql-check"))]
mod tests {
    use super::*;

    #[test]
    fn bounded_input_does_not_accept_a_truncated_prefix() {
        assert_eq!(read_bounded(&b"abcd"[..], 4).unwrap(), b"abcd");
        assert!(read_bounded(&b"abcde"[..], 4).is_err());
    }

    #[test]
    fn encoded_metadata_is_bounded_including_json_escaping() {
        let value = "\u{0000}".repeat(MAX_MESSAGE_BYTES / 6 + 1);
        assert!(encode_bounded(&value).unwrap_err().contains("8 MiB"));
    }

    #[test]
    fn worker_fixture() {
        if std::env::var_os("NAGI_SQL_CHECK_TEST_HANG").is_some() {
            loop {
                thread::park();
            }
        }
    }

    #[test]
    fn deadline_kills_and_reaps_a_worker_with_blocked_input() {
        let started = Instant::now();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "sql_check::tests::worker_fixture", "--nocapture"])
            .env("NAGI_SQL_CHECK_TEST_HANG", "1");
        let error = run_worker(
            command,
            vec![b' '; MAX_MESSAGE_BYTES],
            Duration::from_millis(100),
        )
        .unwrap_err();
        assert!(
            error.contains("deadline") && error.contains("reaped"),
            "{error}"
        );
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn terminating_a_worker_returns_its_reaped_exit_status() {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "sql_check::tests::worker_fixture", "--nocapture"])
            .env("NAGI_SQL_CHECK_TEST_HANG", "1");
        let mut worker = Worker(command.stdout(Stdio::null()).spawn().unwrap());
        assert!(!worker.terminate().unwrap().success());
        assert!(worker.0.try_wait().unwrap().is_some());
    }
}
