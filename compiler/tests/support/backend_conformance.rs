//! Rust/Goで共有する小さなnative観測runner。
//!
//! 既存のRust専用corpus、batch、fuzz、shrinkerは変更しない。ここでは両backendが
//! 対応済みと宣言する少数の入力だけを、同じ期待値と明示した入口で比較する。

use nagic::{ast::Program, check, emit, parser, source};
use serde_json::Value;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

#[cfg(windows)]
use std::ffi::OsStr;

#[cfg(unix)]
use std::os::unix::process::CommandExt as _;
#[cfg(windows)]
use std::os::windows::io::AsRawHandle as _;

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, STILL_ACTIVE},
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    },
    System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
};

#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}

static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Backend {
    Rust,
    Go,
}

/// Execute the private std-only Go kernel without exposing a Nagi runtime API.
/// Require every named native test, not merely a successful `go test` process.
pub fn private_lifecycle_contracts(directory: &Path) -> Result<(), String> {
    let fixture = Fixture::new("private-lifecycle").map_err(|error| error.to_string())?;
    let toolchains = Toolchains::current();
    let mut command = Command::new(&toolchains.go);
    command
        .current_dir(directory)
        .args(["test", "-json", "-p=1", "-count=1", "-timeout=30s", "./..."])
        .env("GOTOOLCHAIN", "local")
        .env("GOWORK", "off")
        .env("CGO_ENABLED", "0");
    let output = capture(
        &mut command,
        Backend::Go,
        Stage::Compile,
        Duration::from_secs(90),
        &fixture,
        "private-lifecycle",
    )
    .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "private Go lifecycle native tests failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let expected = std::collections::BTreeSet::from([
        "TestRegistrationAndColdStart",
        "TestOneForOnePanicRestart",
        "TestTemporaryFailureIsStickyUntilHealthySiblingJoins",
        "TestBusinessReplyIsNotAWorkerFailure",
        "TestPermanentCompletionUsesGlobalRestartBudget",
        "TestRestartBudgetIsSharedAndExactBoundaryExpires",
        "TestStopCancelsPendingRestartAndNeverRestartsCancellation",
        "TestStopRacesRestartRepeatedly",
        "TestBlockedWorkerRetainsRecordUntilActualReturn",
        "TestBlockedCleanupRunsOutsideLocksAndRetainsCompletion",
        "TestDeferredCleanupPrecedesJoinAndFailureSurvivesStop",
        "TestCleanupPanicAndUnstartedStop",
    ]);
    let mut passed = std::collections::BTreeSet::new();
    let mut package_passes = 0;
    for line in output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let row: Value = serde_json::from_slice(line)
            .map_err(|error| format!("invalid Go test JSON: {error}"))?;
        match row["Action"].as_str() {
            Some("skip" | "fail") => {
                return Err(format!(
                    "private native test did not run successfully: {row}"
                ))
            }
            Some("pass") => {
                if let Some(test) = row["Test"].as_str() {
                    if !passed.insert(test.to_owned()) {
                        return Err(format!("duplicate Go native test: {test}"));
                    }
                } else {
                    package_passes += 1;
                }
            }
            _ => {}
        }
    }
    if package_passes != 1
        || passed
            .iter()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>()
            != expected
    {
        return Err(format!("private Go lifecycle execution incomplete: packages={package_passes}, passed={passed:?}"));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum EntryKind {
    High,
    SavedLow,
    HandwrittenLow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Compile,
    Runtime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FaultKind {
    IndexOutOfBounds,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation {
    pub backend: Backend,
    pub entry: EntryKind,
    pub stage: Stage,
    pub status: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expectation {
    RunSuccess { stdout: Vec<u8>, stderr: Vec<u8> },
    RuntimeFault { fault: FaultKind, stdout: Vec<u8> },
}

#[derive(Clone, Debug)]
pub struct SharedCase<'a> {
    pub name: &'a str,
    pub high: &'a str,
    pub handwritten_low: &'a str,
    pub expectation: Expectation,
}

#[derive(Clone, Debug)]
pub struct Toolchains {
    rustc: OsString,
    go: OsString,
    nagic: OsString,
}

impl Toolchains {
    pub fn current() -> Self {
        Self {
            rustc: std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()),
            go: std::env::var_os("NAGI_GO").unwrap_or_else(|| "go".into()),
            nagic: env!("CARGO_BIN_EXE_nagic").into(),
        }
    }

    pub fn with_go(mut self, path: impl Into<OsString>) -> Self {
        self.go = path.into();
        self
    }

    pub fn with_rustc(mut self, path: impl Into<OsString>) -> Self {
        self.rustc = path.into();
        self
    }
}

#[derive(Debug)]
pub enum DriverError {
    Io(String),
    Frontend {
        entry: EntryKind,
        diagnostic: String,
    },
    Infrastructure {
        backend: Backend,
        stage: Stage,
        diagnostic: String,
    },
}

impl std::fmt::Display for DriverError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(diagnostic) => write!(formatter, "fixture I/O: {diagnostic}"),
            Self::Frontend { entry, diagnostic } => {
                write!(formatter, "{entry:?} frontend: {diagnostic}")
            }
            Self::Infrastructure {
                backend,
                stage,
                diagnostic,
            } => write!(
                formatter,
                "{backend:?} {stage:?} infrastructure: {diagnostic}"
            ),
        }
    }
}

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str) -> Result<Self, DriverError> {
        loop {
            let root = std::env::temp_dir().join(format!(
                "nagi-backend-conformance-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&root) {
                Ok(()) => return Ok(Self(root)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(DriverError::Io(error.to_string())),
            }
        }
    }

    fn write(&self, name: &str, bytes: impl AsRef<[u8]>) -> Result<PathBuf, DriverError> {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| DriverError::Io(error.to_string()))?;
        }
        fs::write(&path, bytes).map_err(|error| DriverError::Io(error.to_string()))?;
        Ok(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Debug)]
struct ProcessOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

#[cfg(unix)]
fn isolate_process_tree(command: &mut Command) {
    command.process_group(0);
}

#[cfg(unix)]
fn terminate_process_tree(child: &mut std::process::Child) -> Result<(), String> {
    const SIGNAL_EXISTS: i32 = 0;
    const SIGTERM: i32 = 15;
    const SIGKILL: i32 = 9;
    let process_group = i32::try_from(child.id())
        .map_err(|_| format!("child PID does not fit i32: {}", child.id()))?;
    // `capture` starts every child as its own process-group leader. A
    // negative PID addresses the entire group, including go/compile/link.
    let terminated = unsafe { kill(-process_group, SIGTERM) };
    if terminated != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let started = Instant::now();
    let mut leader_reaped = false;
    while started.elapsed() < Duration::from_millis(500) {
        match child.try_wait() {
            Ok(Some(_)) => {
                leader_reaped = true;
                break;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => return Err(format!("cannot observe process-group leader: {error}")),
        }
    }
    if unsafe { kill(-process_group, SIGNAL_EXISTS) } == 0 {
        let killed = unsafe { kill(-process_group, SIGKILL) };
        if killed != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    if !leader_reaped {
        child
            .wait()
            .map_err(|error| format!("cannot reap process-group leader: {error}"))?;
    }
    Ok(())
}

#[cfg(windows)]
struct WindowsJob(HANDLE);

#[cfg(windows)]
impl WindowsJob {
    fn new() -> Result<Self, String> {
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(format!(
                "CreateJobObjectW failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        let mut information = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                std::ptr::addr_of!(information).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            let error = std::io::Error::last_os_error();
            unsafe {
                CloseHandle(handle);
            }
            return Err(format!("SetInformationJobObject failed: {error}"));
        }
        Ok(Self(handle))
    }

    fn assign(&self, child: &std::process::Child) -> Result<(), String> {
        let process = child.as_raw_handle() as HANDLE;
        if unsafe { AssignProcessToJobObject(self.0, process) } == 0 {
            return Err(format!(
                "AssignProcessToJobObject failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    fn terminate(&self) -> Result<(), String> {
        if unsafe { TerminateJobObject(self.0, 1) } == 0 {
            return Err(format!(
                "TerminateJobObject failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for WindowsJob {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
fn cmd_quote(value: &OsStr) -> Result<String, String> {
    let value = value
        .to_str()
        .ok_or_else(|| format!("command argument is not UTF-8: {value:?}"))?;
    if value.contains(['"', '\r', '\n', '%', '!']) {
        return Err(format!(
            "command argument is unsafe for the gated wrapper: {value:?}"
        ));
    }
    Ok(format!("\"{value}\""))
}

#[cfg(windows)]
fn apply_windows_command_context(source: &Command, target: &mut Command) {
    if let Some(directory) = source.get_current_dir() {
        target.current_dir(directory);
    }
    for (name, value) in source.get_envs() {
        match value {
            Some(value) => {
                target.env(name, value);
            }
            None => {
                target.env_remove(name);
            }
        }
    }
}

#[cfg(windows)]
fn validate_windows_program(original: &Command) -> Result<(), String> {
    // The gated child is cmd.exe, so its successful spawn cannot tell us that
    // the requested rustc/go/native executable exists. Explicit paths are
    // checked directly because `where.exe` treats an existing absolute path as
    // a search pattern and can return exit 2 (including 8.3 temp paths).
    let program = Path::new(original.get_program());
    if program.is_absolute() || program.components().count() > 1 {
        let candidate = if program.is_absolute() {
            program.to_path_buf()
        } else {
            original
                .get_current_dir()
                .map(Path::to_path_buf)
                .or_else(|| std::env::current_dir().ok())
                .ok_or_else(|| "cannot resolve the command working directory".to_owned())?
                .join(program)
        };
        if candidate.is_file()
            || (candidate.extension().is_none() && candidate.with_extension("exe").is_file())
        {
            return Ok(());
        }
        return Err(format!(
            "cannot resolve explicit executable {:?} from command cwd",
            original.get_program()
        ));
    }
    // Bare names follow the wrapper's PATH/PATHEXT lookup under the exact
    // command environment instead of reimplementing Windows search rules.
    let mut lookup = Command::new("where.exe");
    lookup.arg(original.get_program());
    apply_windows_command_context(original, &mut lookup);
    let output = lookup
        .output()
        .map_err(|error| format!("cannot start where.exe for tool lookup: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cannot resolve executable {:?} with the command cwd/PATH/PATHEXT (where.exe exited {:?})",
            original.get_program(),
            output.status.code()
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn gated_windows_command(
    original: &Command,
    fixture: &Fixture,
    label: &str,
) -> Result<(Command, PathBuf), String> {
    validate_windows_program(original)?;
    let gate = fixture.0.join(format!("{label}.gate"));
    let script = fixture.0.join(format!("{label}.cmd"));
    let quoted_gate = cmd_quote(gate.as_os_str())?;
    let mut invocation = cmd_quote(original.get_program())?;
    for argument in original.get_args() {
        invocation.push(' ');
        invocation.push_str(&cmd_quote(argument)?);
    }
    fs::write(
        &script,
        format!(
            "@echo off\r\n:wait_gate\r\nif not exist {quoted_gate} (\r\n  >nul 2>&1 ping.exe 127.0.0.1 -n 2\r\n  goto wait_gate\r\n)\r\n{invocation}\r\nexit /b %errorlevel%\r\n"
        ),
    )
    .map_err(|error| format!("cannot create gated command: {error}"))?;
    let mut command = Command::new("cmd.exe");
    command.args(["/D", "/V:OFF", "/S", "/C"]).arg(&script);
    apply_windows_command_context(original, &mut command);
    Ok((command, gate))
}

fn capture(
    command: &mut Command,
    backend: Backend,
    stage: Stage,
    deadline: Duration,
    fixture: &Fixture,
    label: &str,
) -> Result<ProcessOutput, DriverError> {
    #[cfg(windows)]
    let (mut managed_command, gate) =
        gated_windows_command(command, fixture, label).map_err(|diagnostic| {
            DriverError::Infrastructure {
                backend,
                stage,
                diagnostic,
            }
        })?;
    #[cfg(windows)]
    let command = &mut managed_command;
    let stdout = fixture.0.join(format!("{label}.stdout"));
    let stderr = fixture.0.join(format!("{label}.stderr"));
    command
        .stdout(Stdio::from(
            fs::File::create(&stdout).map_err(|error| DriverError::Io(error.to_string()))?,
        ))
        .stderr(Stdio::from(
            fs::File::create(&stderr).map_err(|error| DriverError::Io(error.to_string()))?,
        ));
    #[cfg(unix)]
    isolate_process_tree(command);
    #[cfg(windows)]
    let job = WindowsJob::new().map_err(|diagnostic| DriverError::Infrastructure {
        backend,
        stage,
        diagnostic,
    })?;
    let mut child = command
        .spawn()
        .map_err(|error| DriverError::Infrastructure {
            backend,
            stage,
            diagnostic: error.to_string(),
        })?;
    #[cfg(windows)]
    {
        if let Err(diagnostic) = job.assign(&child) {
            let killed = child.kill();
            let reaped = child.wait();
            return Err(DriverError::Infrastructure {
                backend,
                stage,
                diagnostic: format!(
                    "{diagnostic}; gated leader cleanup kill={killed:?} wait={reaped:?}"
                ),
            });
        }
        fs::write(&gate, b"assigned").map_err(|error| DriverError::Infrastructure {
            backend,
            stage,
            diagnostic: format!("cannot release gated process after Job assignment: {error}"),
        })?;
    }
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            result => {
                #[cfg(unix)]
                let cleanup = terminate_process_tree(&mut child);
                #[cfg(windows)]
                let cleanup = job.terminate().and_then(|()| {
                    child
                        .wait()
                        .map(|_| ())
                        .map_err(|error| format!("cannot reap Job leader: {error}"))
                });
                return Err(DriverError::Infrastructure {
                    backend,
                    stage,
                    diagnostic: format!(
                        "deadline={}s wait={result:?} cleanup={cleanup:?}",
                        deadline.as_secs()
                    ),
                });
            }
        }
    };
    #[cfg(windows)]
    job.terminate()
        .map_err(|diagnostic| DriverError::Infrastructure {
            backend,
            stage,
            diagnostic: format!("cannot clean completed Job descendants: {diagnostic}"),
        })?;
    Ok(ProcessOutput {
        status,
        stdout: fs::read(stdout).map_err(|error| DriverError::Io(error.to_string()))?,
        stderr: fs::read(stderr).map_err(|error| DriverError::Io(error.to_string()))?,
    })
}

fn frontend_error(entry: EntryKind, diagnostic: impl ToString) -> DriverError {
    DriverError::Frontend {
        entry,
        diagnostic: diagnostic.to_string(),
    }
}

fn checked_rust(path: &Path, entry: EntryKind) -> Result<String, DriverError> {
    let high = entry == EntryKind::High;
    let mut loaded = source::load(path, high).map_err(|error| frontend_error(entry, error))?;
    check::check(&mut loaded.program).map_err(|error| frontend_error(entry, error))?;
    let provenance = loaded.provenance();
    let program = if high {
        let lowered = emit::low_with_lines(&loaded.program);
        let mut low =
            parser::parse(&lowered.text, false).map_err(|error| frontend_error(entry, error))?;
        lowered
            .restore_lines(&mut low)
            .map_err(|error| frontend_error(entry, error))?;
        check::check(&mut low).map_err(|error| frontend_error(entry, error))?;
        low
    } else {
        loaded.program
    };
    let checked = check::finalize(program, Program::default(), provenance)
        .map_err(|error| frontend_error(entry, error))?;
    emit::rust(&checked).map_err(|error| frontend_error(entry, error))
}

fn saved_low(path: &Path) -> Result<String, DriverError> {
    let mut loaded =
        source::load(path, true).map_err(|error| frontend_error(EntryKind::High, error))?;
    check::check(&mut loaded.program).map_err(|error| frontend_error(EntryKind::High, error))?;
    Ok(emit::low(&loaded.program))
}

fn source_name(entry: EntryKind) -> &'static str {
    match entry {
        EntryKind::High => "main.nagi",
        EntryKind::SavedLow => "saved.low",
        EntryKind::HandwrittenLow => "handwritten.low",
    }
}

fn rust_observation(
    fixture: &Fixture,
    entry: EntryKind,
    tools: &Toolchains,
) -> Result<Observation, DriverError> {
    let rust = checked_rust(&fixture.0.join(source_name(entry)), entry)?;
    let source = fixture.write(&format!("rust-{entry:?}.rs"), rust)?;
    let binary = fixture
        .0
        .join(format!("rust-{entry:?}{}", std::env::consts::EXE_SUFFIX));
    let mut compiler = Command::new(&tools.rustc);
    compiler
        .args(["--edition=2021", "-D", "warnings"])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .current_dir(&fixture.0);
    let compiled = capture(
        &mut compiler,
        Backend::Rust,
        Stage::Compile,
        Duration::from_secs(30),
        fixture,
        &format!("rust-{entry:?}-compile"),
    )?;
    if !compiled.status.success() {
        return Ok(Observation {
            backend: Backend::Rust,
            entry,
            stage: Stage::Compile,
            status: compiled.status.code(),
            stdout: compiled.stdout,
            stderr: compiled.stderr,
        });
    }
    let mut native = Command::new(binary);
    native.current_dir(&fixture.0);
    let ran = capture(
        &mut native,
        Backend::Rust,
        Stage::Runtime,
        Duration::from_secs(10),
        fixture,
        &format!("rust-{entry:?}-runtime"),
    )?;
    Ok(Observation {
        backend: Backend::Rust,
        entry,
        stage: Stage::Runtime,
        status: ran.status.code(),
        stdout: ran.stdout,
        stderr: ran.stderr,
    })
}

fn latest_executable(out: &Path) -> Result<PathBuf, DriverError> {
    let apps = out.join(".nagi/apps");
    let app = fs::read_dir(&apps)
        .map_err(|error| DriverError::Io(error.to_string()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("nagi-go-"))
        })
        .ok_or_else(|| DriverError::Io(format!("missing Go app in {}", apps.display())))?;
    let latest: Value = serde_json::from_slice(
        &fs::read(app.join("latest.json")).map_err(|error| DriverError::Io(error.to_string()))?,
    )
    .map_err(|error| DriverError::Io(error.to_string()))?;
    let executable = latest["executable"]
        .as_str()
        .ok_or_else(|| DriverError::Io("latest executable is missing".into()))?;
    Ok(app.join(executable))
}

fn go_observation(
    fixture: &Fixture,
    entry: EntryKind,
    tools: &Toolchains,
) -> Result<Observation, DriverError> {
    let mut version = Command::new(&tools.go);
    version.arg("version").current_dir(&fixture.0);
    let version = capture(
        &mut version,
        Backend::Go,
        Stage::Compile,
        Duration::from_secs(10),
        fixture,
        &format!("go-{entry:?}-version"),
    )?;
    if !version.status.success() {
        return Err(DriverError::Infrastructure {
            backend: Backend::Go,
            stage: Stage::Compile,
            diagnostic: format!(
                "go version exited {:?}: {}{}",
                version.status.code(),
                String::from_utf8_lossy(&version.stdout),
                String::from_utf8_lossy(&version.stderr)
            ),
        });
    }
    let out = fixture.0.join(format!("go-{entry:?}"));
    let mut compiler = Command::new(&tools.nagic);
    compiler
        .args([
            "build",
            source_name(entry),
            "--no-project",
            "--backend",
            "go",
            "--profile",
            "dev",
            "--out",
        ])
        .arg(&out)
        .current_dir(&fixture.0)
        .env("NAGI_GO", &tools.go)
        .env_remove("NAGI_RUN_RETENTION");
    let compiled = capture(
        &mut compiler,
        Backend::Go,
        Stage::Compile,
        Duration::from_secs(60),
        fixture,
        &format!("go-{entry:?}-compile"),
    )?;
    if !compiled.status.success() {
        return Ok(Observation {
            backend: Backend::Go,
            entry,
            stage: Stage::Compile,
            status: compiled.status.code(),
            stdout: compiled.stdout,
            stderr: compiled.stderr,
        });
    }
    let executable = latest_executable(&out)?;
    let mut native = Command::new(executable);
    native.current_dir(&fixture.0);
    let ran = capture(
        &mut native,
        Backend::Go,
        Stage::Runtime,
        Duration::from_secs(10),
        fixture,
        &format!("go-{entry:?}-runtime"),
    )?;
    Ok(Observation {
        backend: Backend::Go,
        entry,
        stage: Stage::Runtime,
        status: ran.status.code(),
        stdout: ran.stdout,
        stderr: ran.stderr,
    })
}

fn primary_fault(backend: Backend, stderr: &[u8]) -> Option<FaultKind> {
    let stderr = std::str::from_utf8(stderr).ok()?;
    match backend {
        Backend::Rust => stderr
            .lines()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|lines| {
                lines[0].starts_with("thread '")
                    && lines[0].contains(" panicked at ")
                    && lines[0].ends_with(':')
                    && lines[1] == "index out of bounds: the len is 1 but the index is 2"
            })
            .then_some(FaultKind::IndexOutOfBounds),
        Backend::Go
            if stderr
                .lines()
                .any(|line| line == "panic: index out of bounds") =>
        {
            Some(FaultKind::IndexOutOfBounds)
        }
        _ => None,
    }
}

pub fn verify(expectation: &Expectation, observation: &Observation) -> Result<(), String> {
    match expectation {
        Expectation::RunSuccess { stdout, stderr } => {
            if observation.stage != Stage::Runtime
                || observation.status != Some(0)
                || observation.stdout != *stdout
                || observation.stderr != *stderr
            {
                return Err(format!("expected exact success; got {observation:?}"));
            }
        }
        Expectation::RuntimeFault { fault, stdout } => {
            if observation.stage != Stage::Runtime
                || observation.status == Some(0)
                || observation.status.is_none()
                || observation.stdout != *stdout
                || primary_fault(observation.backend, &observation.stderr) != Some(*fault)
            {
                return Err(format!(
                    "expected {fault:?} runtime fault; got {observation:?}"
                ));
            }
        }
    }
    Ok(())
}

pub fn run_case(
    case: &SharedCase<'_>,
    entries: &[EntryKind],
    tools: &Toolchains,
) -> Result<Vec<Observation>, DriverError> {
    let fixture = Fixture::new(case.name)?;
    let high = fixture.write("main.nagi", case.high)?;
    fixture.write("saved.low", saved_low(&high)?)?;
    fixture.write("handwritten.low", case.handwritten_low)?;
    let mut observations = Vec::new();
    for entry in entries {
        observations.push(rust_observation(&fixture, *entry, tools)?);
        observations.push(go_observation(&fixture, *entry, tools)?);
    }
    Ok(observations)
}

pub fn assert_case(
    case: &SharedCase<'_>,
    entries: &[EntryKind],
) -> Result<Vec<Observation>, String> {
    let observations = run_case(case, entries, &Toolchains::current())
        .map_err(|error| format!("{} driver error: {error}", case.name))?;
    let expected_count = entries.len() * 2;
    if observations.len() != expected_count {
        return Err(format!(
            "{} expected {expected_count} observations, got {}",
            case.name,
            observations.len()
        ));
    }
    for observation in &observations {
        verify(&case.expectation, observation)
            .map_err(|error| format!("{}: {error}", case.name))?;
    }
    for entry in entries {
        let rust = observations
            .iter()
            .find(|observation| observation.entry == *entry && observation.backend == Backend::Rust)
            .ok_or_else(|| format!("{} missing Rust {entry:?}", case.name))?;
        let go = observations
            .iter()
            .find(|observation| observation.entry == *entry && observation.backend == Backend::Go)
            .ok_or_else(|| format!("{} missing Go {entry:?}", case.name))?;
        if rust.stdout != go.stdout {
            return Err(format!(
                "{} {entry:?} backend stdout differs: Rust={:?}, Go={:?}",
                case.name, rust.stdout, go.stdout
            ));
        }
    }
    Ok(observations)
}

pub fn assert_go_rejected(
    name: &str,
    high: &str,
    handwritten_low: &str,
    entry: EntryKind,
    diagnostic: &str,
    line: usize,
) -> Result<(), String> {
    let case = SharedCase {
        name,
        high,
        handwritten_low,
        expectation: Expectation::RunSuccess {
            stdout: Vec::new(),
            stderr: Vec::new(),
        },
    };
    let fixture = Fixture::new(name).map_err(|error| error.to_string())?;
    let high_path = fixture
        .write("main.nagi", case.high)
        .map_err(|error| error.to_string())?;
    fixture
        .write(
            "saved.low",
            saved_low(&high_path).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    fixture
        .write("handwritten.low", case.handwritten_low)
        .map_err(|error| error.to_string())?;
    let tools = Toolchains::current();
    let rust = rust_observation(&fixture, entry, &tools).map_err(|error| error.to_string())?;
    if rust.stage != Stage::Runtime || rust.status != Some(0) {
        return Err(format!("Rust should run supported source: {rust:?}"));
    }
    let go = go_observation(&fixture, entry, &tools).map_err(|error| error.to_string())?;
    let stderr = String::from_utf8_lossy(&go.stderr);
    if go.stage != Stage::Compile
        || go.status == Some(0)
        || !stderr.contains(diagnostic)
        || !stderr.contains(&format!("{}:{line}", source_name(entry)))
    {
        return Err(format!("Go rejection mismatch: {go:?}"));
    }
    Ok(())
}

pub fn assert_frontend_rejected(
    name: &str,
    high: &str,
    handwritten_low: &str,
    diagnostic: &str,
    line: usize,
) -> Result<(), String> {
    let fixture = Fixture::new(name).map_err(|error| error.to_string())?;
    fixture
        .write("main.nagi", high)
        .map_err(|error| error.to_string())?;
    fixture
        .write("handwritten.low", handwritten_low)
        .map_err(|error| error.to_string())?;
    for (entry, high) in [(EntryKind::High, true), (EntryKind::HandwrittenLow, false)] {
        let path = fixture.0.join(source_name(entry));
        let mut loaded = source::load(&path, high)
            .map_err(|error| format!("{entry:?} parsed as the wrong stage: {error}"))?;
        let error = check::check(&mut loaded.program)
            .expect_err("frontend-negative fixture unexpectedly passed the checker");
        if !error.starts_with(&format!("line {line}:")) || !error.contains(diagnostic) {
            return Err(format!("{entry:?} rejection mismatch: {error}"));
        }
    }
    Ok(())
}

pub fn missing_toolchains_are_infrastructure(case: &SharedCase<'_>) -> Result<(), String> {
    let fixture = Fixture::new("missing-toolchains").map_err(|error| error.to_string())?;
    let high = fixture
        .write("main.nagi", case.high)
        .map_err(|error| error.to_string())?;
    fixture
        .write(
            "saved.low",
            saved_low(&high).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    fixture
        .write("handwritten.low", case.handwritten_low)
        .map_err(|error| error.to_string())?;
    let missing_go = fixture.0.join("missing-go");
    let go = go_observation(
        &fixture,
        EntryKind::High,
        &Toolchains::current().with_go(missing_go),
    );
    if !matches!(
        go,
        Err(DriverError::Infrastructure {
            backend: Backend::Go,
            stage: Stage::Compile,
            ..
        })
    ) {
        return Err(format!("missing Go was misclassified: {go:?}"));
    }
    let missing_rustc = fixture.0.join("missing-rustc");
    let rust = rust_observation(
        &fixture,
        EntryKind::High,
        &Toolchains::current().with_rustc(missing_rustc),
    );
    if !matches!(
        rust,
        Err(DriverError::Infrastructure {
            backend: Backend::Rust,
            stage: Stage::Compile,
            ..
        })
    ) {
        return Err(format!("missing rustc was misclassified: {rust:?}"));
    }
    Ok(())
}

pub fn fake_observation(
    backend: Backend,
    stage: Stage,
    status: Option<i32>,
    stdout: impl AsRef<[u8]>,
    stderr: impl AsRef<[u8]>,
) -> Observation {
    Observation {
        backend,
        entry: EntryKind::High,
        stage,
        status,
        stdout: stdout.as_ref().to_vec(),
        stderr: stderr.as_ref().to_vec(),
    }
}

#[cfg(unix)]
pub fn deadline_reaps_descendant() -> Result<(), String> {
    let fixture = Fixture::new("deadline-tree").map_err(|error| error.to_string())?;
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(
            "child=''; trap 'kill \"$child\" 2>/dev/null || true; wait \"$child\" 2>/dev/null || true; exit 143' TERM; sleep 300 & child=$!; printf '%s\\n' \"$child\" > child.pid; wait \"$child\"",
        )
        .current_dir(&fixture.0);
    let result = capture(
        &mut command,
        Backend::Go,
        Stage::Compile,
        Duration::from_secs(1),
        &fixture,
        "deadline-tree",
    );
    let diagnostic = match result {
        Err(DriverError::Infrastructure { diagnostic, .. }) => diagnostic,
        other => return Err(format!("hanging process tree did not time out: {other:?}")),
    };
    if !diagnostic.contains("cleanup=Ok(())") {
        return Err(format!("process tree cleanup failed: {diagnostic}"));
    }
    let pid: i32 = fs::read_to_string(fixture.0.join("child.pid"))
        .map_err(|error| format!("missing descendant PID: {error}"))?
        .trim()
        .parse()
        .map_err(|error| format!("invalid descendant PID: {error}"))?;
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(1) {
        if unsafe { kill(pid, 0) } != 0 {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Err(format!("descendant {pid} survived process-group cleanup"))
}

#[cfg(windows)]
pub fn deadline_reaps_descendant() -> Result<(), String> {
    let fixture = Fixture::new("job-tree").map_err(|error| error.to_string())?;
    let pid_path = fixture.0.join("child.pid");
    let parent = format!(
        "$p=Start-Process powershell.exe -PassThru -ArgumentList @('-NoProfile','-Command','Start-Sleep -Seconds 300'); Set-Content -NoNewline -Path '{}' -Value $p.Id; exit 0",
        pid_path.display()
    );
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-Command", &parent])
        .current_dir(&fixture.0);
    let output = capture(
        &mut command,
        Backend::Go,
        Stage::Compile,
        Duration::from_secs(10),
        &fixture,
        "job-tree",
    )
    .map_err(|error| format!("leader-exit Job regression failed: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "leader-exit fixture failed {:?}: {}{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let pid: u32 = fs::read_to_string(pid_path)
        .map_err(|error| format!("missing descendant PID: {error}"))?
        .trim()
        .parse()
        .map_err(|error| format!("invalid descendant PID: {error}"))?;
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(2) {
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return Ok(());
        }
        let mut exit_code = 0;
        let inspected = unsafe { GetExitCodeProcess(handle, &mut exit_code) };
        unsafe {
            CloseHandle(handle);
        }
        if inspected == 0 {
            return Err(format!(
                "cannot inspect descendant {pid}: {}",
                std::io::Error::last_os_error()
            ));
        }
        // Windows can still open a terminated process while another handle
        // keeps its process object alive. Only STILL_ACTIVE proves that the
        // descendant escaped Job cleanup.
        if exit_code != STILL_ACTIVE as u32 {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Err(format!("descendant {pid} survived completed Job cleanup"))
}
