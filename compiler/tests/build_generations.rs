//! ADR 007 regression contract: stable package/cache identity, immutable successful
//! generations, and one advisory writer lock per canonical logical output.
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

static ID: AtomicU64 = AtomicU64::new(0);
const DEADLINE: Duration = Duration::from_secs(60);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "nagi generations 凪 {} {}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let f = Self(root);
        f.write(
            "runtime/Cargo.toml",
            "[package]\nname='nagi-runtime'\nversion='0.1.0'\nedition='2021'\n[workspace]\n",
        );
        f.write("runtime/src/lib.rs", "");
        f.source("main.nagi", "old");
        f
    }
    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn source(&self, name: &str, value: &str) {
        self.write(name, &format!("def main():\n    print(\"{value}\")\n"));
    }
    fn cli(&self, verb: &str, source: &str, out: &str) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_nagic"));
        c.current_dir(&self.0)
            .args([verb, source, "--no-project", "--out", out])
            .env("NAGI_ROOT", &self.0)
            .env("CARGO_NET_OFFLINE", "true")
            .env("NAGI_NATIVE_TARGET_DIR", self.0.join("cache"));
        c
    }
    fn build(&self) -> PathBuf {
        native(&bounded_output(self.cli("build", "main.nagi", "out")))
    }
    fn app(&self, out: &str) -> PathBuf {
        self.0
            .join(out)
            .join(".nagi/apps")
            .join(package(&self.0.join(out)))
    }
    fn latest(&self, out: &str) -> (Vec<u8>, serde_json::Value) {
        let bytes = fs::read(self.app(out).join("latest.json"))
            .expect("a successful build must publish complete latest metadata");
        let json = serde_json::from_slice(&bytes).unwrap();
        (bytes, json)
    }
    fn wrapper(&self) -> PathBuf {
        self.write("cargo-wrapper.rs", CARGO_WRAPPER);
        let dir = self.0.join("wrapper");
        fs::create_dir_all(&dir).unwrap();
        let mut command = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
        command
            .arg(self.0.join("cargo-wrapper.rs"))
            .args(["--edition=2021", "-o"])
            .arg(dir.join(format!("cargo{}", std::env::consts::EXE_SUFFIX)));
        let result = bounded_output(command);
        success(&result);
        dir
    }
    fn wrapped(
        &self,
        wrapper: &Path,
        verb: &str,
        source: &str,
        out: &str,
        signals: &str,
        mode: &str,
    ) -> Command {
        let mut c = self.cli(verb, source, out);
        c.env(
            "PATH",
            std::env::join_paths(std::iter::once(wrapper.to_path_buf()).chain(
                std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
            ))
            .unwrap(),
        )
        .env("NAGI_TEST_REAL_CARGO", env!("CARGO"))
        .env("NAGI_TEST_SIGNALS", self.0.join(signals))
        .env("NAGI_TEST_MODE", mode)
        .env("NAGI_TEST_OUT", self.0.join(out));
        fs::create_dir_all(self.0.join(signals)).unwrap();
        c
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Running {
    child: Option<Child>,
    signals: Option<PathBuf>,
    wrapper_owned: bool,
    stderr: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    stderr_reader: Option<thread::JoinHandle<()>>,
}
impl Running {
    fn spawn(command: Command, signals: PathBuf) -> Self {
        Self::spawn_inner(command, Some(signals), false)
    }
    fn wrapper(command: Command, signals: PathBuf) -> Self {
        Self::spawn_inner(command, Some(signals), true)
    }
    fn spawn_inner(mut command: Command, signals: Option<PathBuf>, wrapper_owned: bool) -> Self {
        if let Some(path) = &signals {
            fs::create_dir_all(path).unwrap();
        }
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stderr = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = stderr.clone();
        let marker = signals.clone();
        let stream = child.stderr.take().unwrap();
        let reader = thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            let mut stream = BufReader::new(stream);
            let mut line = Vec::new();
            loop {
                line.clear();
                match stream.read_until(b'\n', &mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        captured.lock().unwrap().extend_from_slice(&line);
                        if line.starts_with(b"waiting for Nagi run lease: ") {
                            if let Some(path) = &marker {
                                let _ = fs::write(path.join("lease-waiting"), "");
                            }
                        }
                        if line.starts_with(b"waiting for output lock: ") {
                            if let Some(path) = &marker {
                                let _ = fs::write(path.join("waiting"), "");
                            }
                        }
                    }
                    Err(error) => {
                        captured
                            .lock()
                            .unwrap()
                            .extend_from_slice(error.to_string().as_bytes());
                        break;
                    }
                }
            }
        });
        Self {
            child: Some(child),
            signals,
            wrapper_owned,
            stderr,
            stderr_reader: Some(reader),
        }
    }
    fn signals(&self) -> &Path {
        self.signals.as_deref().unwrap()
    }
    fn captured_error(&self) -> String {
        String::from_utf8_lossy(&self.stderr.lock().unwrap()).into_owned()
    }
    fn wait_for(&mut self, name: &str) {
        let until = Instant::now() + DEADLINE;
        while !self.signals().join(name).exists() {
            if self.child.as_mut().unwrap().try_wait().unwrap().is_some() {
                panic!("child exited before {name}: {}", self.captured_error());
            }
            assert!(
                Instant::now() < until,
                "barrier {name} timed out: {}",
                self.captured_error()
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
    fn release(&self) {
        fs::write(self.signals().join("release"), "").unwrap();
    }
    fn output(mut self) -> Output {
        let until = Instant::now() + DEADLINE;
        loop {
            if self.child.as_mut().unwrap().try_wait().unwrap().is_some() {
                break;
            }
            assert!(Instant::now() < until, "child did not finish after release");
            thread::sleep(Duration::from_millis(10));
        }
        let mut output = self.child.take().unwrap().wait_with_output().unwrap();
        self.stderr_reader.take().unwrap().join().unwrap();
        output.stderr = self.stderr.lock().unwrap().clone();
        output
    }
    fn kill_parent(&mut self) {
        let child = self.child.as_mut().unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(signals) = &self.signals {
            let _ = fs::write(signals.join("release"), "");
        }
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        if self.wrapper_owned {
            let signals = self.signals();
            if signals.join("entered").exists() {
                let until = Instant::now() + DEADLINE;
                while !signals.join("done").exists() && Instant::now() < until {
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }
        if let Some(reader) = self.stderr_reader.take() {
            let _ = reader.join();
        }
    }
}
fn bounded_output(command: Command) -> Output {
    Running::spawn_inner(command, None, false).output()
}
fn os_writer_is_held(f: &Fixture, out: &str) {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(f.0.join(out).join(".nagi-write.lock"))
        .expect("writer must create its permanent OS lock");
    assert!(
        matches!(file.try_lock(), Err(std::fs::TryLockError::WouldBlock)),
        "active generation must hold the OS lock"
    );
}
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn native(output: &Output) -> PathBuf {
    success(output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let p = PathBuf::from(
        stderr
            .lines()
            .find_map(|s| s.strip_prefix("native: "))
            .expect("successful build must report native artifact"),
    );
    assert!(p.is_file(), "{}", p.display());
    fs::canonicalize(p).unwrap()
}
fn package(out: &Path) -> String {
    let m: toml::Value =
        toml::from_str(&fs::read_to_string(out.join("Cargo.toml")).unwrap()).unwrap();
    m["package"]["name"].as_str().unwrap().to_string()
}
fn run(binary: &Path) -> String {
    let output = bounded_output(Command::new(binary));
    success(&output);
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}
fn assert_inside(child: &Path, parent: &Path) {
    assert!(
        fs::canonicalize(child)
            .unwrap()
            .starts_with(fs::canonicalize(parent).unwrap()),
        "{} must be inside {}",
        child.display(),
        parent.display()
    );
}
fn assert_os_path(record: &serde_json::Value, path: &Path) {
    #[cfg(unix)]
    let expected = {
        use std::os::unix::ffi::OsStrExt;
        serde_json::json!({"encoding":"unix_bytes","units":path.as_os_str().as_bytes()})
    };
    #[cfg(windows)]
    let expected = {
        use std::os::windows::ffi::OsStrExt;
        let units: Vec<u16> = path.as_os_str().encode_wide().collect();
        serde_json::json!({"encoding":"windows_utf16","units":units})
    };
    #[cfg(not(any(unix, windows)))]
    let expected =
        serde_json::json!({"encoding":"encoded_bytes","units":path.as_os_str().as_encoded_bytes()});
    assert_eq!(
        record, &expected,
        "snapshot must preserve the exact OS path"
    );
}
fn validate_latest(f: &Fixture, out: &str, native: &Path) -> serde_json::Value {
    let (_, m) = f.latest(out);
    assert_eq!(m["schema_version"], 1);
    assert_eq!(m["app_id"].as_str().unwrap(), package(&f.0.join(out)));
    let app = f.app(out);
    let generation = m["generation"].as_str().unwrap();
    let published = app.join("generations").join(generation);
    let executable = app.join(m["executable"].as_str().unwrap());
    assert_eq!(fs::canonicalize(executable).unwrap(), native);
    assert_inside(native, &published);
    for name in ["manifest", "low", "rust", "sources", "provenance", "lock"] {
        let path = app.join(m[name].as_str().unwrap());
        assert!(path.is_file(), "{name}: {}", path.display());
        assert_inside(&path, &published);
    }
    let manifest_path = app.join(m["manifest"].as_str().unwrap());
    let manifest: toml::Value =
        toml::from_str(&fs::read_to_string(manifest_path).unwrap()).unwrap();
    assert_eq!(
        manifest["package"]["name"].as_str().unwrap(),
        m["app_id"].as_str().unwrap()
    );
    assert_eq!(manifest["package"]["autobins"].as_bool(), Some(false));
    assert_eq!(manifest["bin"].as_array().unwrap().len(), 1);
    assert_eq!(
        manifest["bin"][0]["name"].as_str().unwrap(),
        native.file_stem().unwrap().to_str().unwrap()
    );
    m
}

#[test]
fn rebuild_publishes_new_generation_and_preserves_old_executable_and_snapshot() {
    let f = Fixture::new();
    let first = f.build();
    assert_eq!(run(&first), "old");
    let first_bytes = fs::read(&first).unwrap();
    let old_package = package(&f.0.join("out"));
    let m1 = validate_latest(&f, "out", &first);
    let old_low = f.app("out").join(m1["low"].as_str().unwrap());
    let old_low_bytes = fs::read(&old_low).unwrap();
    let snapshots: Vec<_> = [
        "executable",
        "manifest",
        "low",
        "rust",
        "sources",
        "provenance",
        "lock",
    ]
    .into_iter()
    .map(|field| {
        let path = f.app("out").join(m1[field].as_str().unwrap());
        let bytes = fs::read(&path).unwrap();
        (path, bytes)
    })
    .collect();
    let sources: serde_json::Value = serde_json::from_slice(
        &fs::read(f.app("out").join(m1["sources"].as_str().unwrap())).unwrap(),
    )
    .unwrap();
    assert_eq!(sources["schema_version"], 1);
    let canonical = fs::canonicalize(f.0.join("main.nagi")).unwrap();
    let source = sources["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["path"].as_str() == canonical.to_str())
        .unwrap();
    assert_eq!(source["text"], fs::read_to_string(&canonical).unwrap());
    assert_eq!(source["kind"], "generated_low");
    assert_os_path(&source["path_os"], &canonical);
    assert!(
        source["module"].is_string(),
        "loaded source retains its nominal ModuleId"
    );
    let provenance: serde_json::Value = serde_json::from_slice(
        &fs::read(f.app("out").join(m1["provenance"].as_str().unwrap())).unwrap(),
    )
    .unwrap();
    assert_eq!(provenance["schema_version"], 1);
    for line in provenance["rust_lines"].as_array().unwrap() {
        if line["path"].as_str() == canonical.to_str() {
            assert_os_path(&line["path_os"], &canonical);
        }
        assert!(
            line.get("module").is_some(),
            "synthetic/unknown origins use explicit null module"
        );
        assert!(matches!(
            line["kind"].as_str(),
            Some(
                "generated_low"
                    | "user_low"
                    | "native_low"
                    | "replacement"
                    | "synthetic"
                    | "unknown"
            )
        ));
    }
    assert!(provenance["rust_lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line["path"].as_str() == canonical.to_str()
            && line["kind"] == "generated_low"
            && line["module"] == source["module"]));
    f.write(
        "out/src/main.rs",
        "caller changes projection before rebuild",
    );
    f.write(
        "out/generated.low",
        "caller changes Low projection before rebuild",
    );
    f.source("main.nagi", "new");
    let second = f.build();
    let m2 = validate_latest(&f, "out", &second);
    assert_eq!(package(&f.0.join("out")), old_package);
    assert_ne!(first, second);
    assert_ne!(m1["generation"], m2["generation"]);
    assert_eq!(run(&second), "new");
    assert_eq!(run(&first), "old");
    assert!(fs::read(first).unwrap() == first_bytes);
    assert_eq!(fs::read(old_low).unwrap(), old_low_bytes);
    for (path, bytes) in snapshots {
        assert!(
            fs::read(&path).unwrap() == bytes,
            "immutable snapshot changed: {}",
            path.display()
        );
    }
    // Published bytes must be a copy, never a link to the mutable Cargo cache.
    let cached = f.0.join("cache/release").join(second.file_name().unwrap());
    assert!(cached.is_file());
    let published_bytes = fs::read(&second).unwrap();
    fs::write(cached, b"changed mutable cache").unwrap();
    assert!(fs::read(&second).unwrap() == published_bytes);
    assert_eq!(run(&second), "new");
}

#[test]
fn a_running_old_executable_is_not_replaced_by_a_new_build() {
    let f = Fixture::new();
    f.write(
        "bridge.rs",
        r#"pub fn hold() {
    let dir = std::path::PathBuf::from(std::env::var_os("NAGI_TEST_SIGNALS").unwrap());
    std::fs::write(dir.join("entered"), "").unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !dir.join("release").exists() {
        assert!(std::time::Instant::now() < until);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}"#,
    );
    f.write(
        "main.nagi",
        "@rust(\"native::hold\")\nextern def hold()\ndef main():\n    print(\"old\")\n    hold()\n",
    );
    let mut first_command = f.cli("build", "main.nagi", "out");
    first_command.args(["--rust", "bridge.rs"]);
    let first = native(&bounded_output(first_command));
    let before = fs::read(&first).unwrap();
    let signals = f.0.join("running");
    fs::create_dir_all(&signals).unwrap();
    let mut old = Command::new(&first);
    old.env("NAGI_TEST_SIGNALS", &signals);
    let mut old = Running::spawn(old, signals);
    old.wait_for("entered");
    f.source("main.nagi", "new");
    let second = f.build();
    assert_ne!(
        first, second,
        "a running success generation cannot be reused"
    );
    assert!(fs::read(first).unwrap() == before);
    assert_eq!(run(&second), "new");
    old.release();
    let output = old.output();
    success(&output);
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "old");
}

#[test]
fn managed_successful_runs_retire_old_generations_but_default_builds_keep_theirs() {
    let f = Fixture::new();
    let default = f.build();
    let managed_run = || {
        let mut command = f.cli("run", "main.nagi", "out");
        command.env("NAGI_RUN_RETENTION", "latest");
        let output = bounded_output(command);
        let executable = native(&output);
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "old");
        executable
    };
    let first = managed_run();
    let second = managed_run();
    assert_ne!(first, second);
    assert!(
        !first.parent().unwrap().exists(),
        "a completed managed run must be retired after the next successful run"
    );
    assert!(second.is_file(), "the latest generation must stay runnable");
    assert!(
        default.is_file(),
        "ordinary CLI build artifacts are not an IDE-owned cache"
    );
    assert_eq!(run(&default), "old");
}

fn managed_command(f: &Fixture, source: &str) -> Command {
    managed_command_at(f, source, "out")
}

fn managed_command_at(f: &Fixture, source: &str, out: &str) -> Command {
    let mut command = f.cli("run", source, out);
    command.env("NAGI_RUN_RETENTION", "latest");
    command
}

fn managed_project_command(f: &Fixture, project: &str, out: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nagic"));
    command
        .current_dir(&f.0)
        .args(["run", "--project", project, "--out", out])
        .env("NAGI_ROOT", &f.0)
        .env("CARGO_NET_OFFLINE", "true")
        .env("NAGI_NATIVE_TARGET_DIR", f.0.join("cache"))
        .env("NAGI_RUN_RETENTION", "latest");
    command
}

fn run_lease(binary: &Path) -> PathBuf {
    let generation = binary.parent().unwrap();
    generation
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("run-retention")
        .join(format!(
            "{}.lease",
            generation.file_name().unwrap().to_str().unwrap()
        ))
}

#[cfg(unix)]
fn test_file_identity(path: &Path) -> [u64; 2] {
    let metadata = fs::metadata(path).unwrap();
    use std::os::unix::fs::MetadataExt;
    [metadata.dev(), metadata.ino()]
}

fn app_for(binary: &Path) -> &Path {
    binary
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("native executable is inside an application generation")
}

fn lease_header(binary: &Path, state: &str) -> String {
    let generation = binary.parent().unwrap();
    let app = app_for(binary);
    format!(
        "{state}:NAGI-RUN-2:{}:{}\n",
        app.file_name().unwrap().to_str().unwrap(),
        generation.file_name().unwrap().to_str().unwrap()
    )
}

fn retire_lease(binary: &Path) {
    use std::io::{Seek, SeekFrom, Write};

    let mut lease = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(run_lease(binary))
        .unwrap();
    lease.lock().unwrap();
    lease.set_len(0).unwrap();
    lease.seek(SeekFrom::Start(0)).unwrap();
    lease
        .write_all(lease_header(binary, "X").as_bytes())
        .unwrap();
    lease.sync_all().unwrap();
}

#[test]
fn managed_low_runs_and_failed_builds_keep_the_last_good_generation() {
    for text in [
        "fn main() {\n    print(\"old\");\n}\n",
        "def main():\n    print(\"old\")\n",
    ] {
        let f = Fixture::new();
        let source = if text.contains('{') {
            "main.low"
        } else {
            "main.nagi"
        };
        f.write(source, text);
        let first = native(&bounded_output(managed_command(&f, source)));
        let latest = f.latest("out").0;
        let wrapper = f.wrapper();
        let mut failure = f.wrapped(&wrapper, "run", source, "out", "failure", "fail");
        failure.env("NAGI_RUN_RETENTION", "latest");
        let failure = bounded_output(failure);
        assert!(!failure.status.success());
        assert!(String::from_utf8_lossy(&failure.stderr).contains("Build failed."));
        assert_eq!(f.latest("out").0, latest);
        assert!(first.is_file());
        let second = native(&bounded_output(managed_command(&f, source)));
        assert!(!first.parent().unwrap().exists());
        assert_eq!(run(&second), "old");
    }
}

#[test]
fn managed_program_failure_does_not_retire_old_successes() {
    let f = Fixture::new();
    let first = native(&bounded_output(managed_command(&f, "main.nagi")));
    f.write("bridge.rs", "pub fn fail() { std::process::exit(7); }\n");
    f.write(
        "main.nagi",
        "@rust(\"native::fail\")\nextern def fail()\ndef main():\n    fail()\n",
    );
    let mut failure = managed_command(&f, "main.nagi");
    failure.args(["--rust", "bridge.rs"]);
    let failure = bounded_output(failure);
    assert!(!failure.status.success());
    assert!(String::from_utf8_lossy(&failure.stderr).contains("program exited:"));
    assert!(first.is_file());
    let (_, failed) = f.latest("out");
    let failed = f.app("out").join(failed["executable"].as_str().unwrap());
    let lease = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(run_lease(&failed))
        .unwrap();
    assert!(
        lease.try_lock().is_ok(),
        "process::exit must release the native kernel lease"
    );
    drop(lease);
    f.source("main.nagi", "recovered");
    let recovered = native(&bounded_output(managed_command(&f, "main.nagi")));
    assert!(!first.parent().unwrap().exists());
    assert_eq!(run(&recovered), "recovered");
}

const HOLD_RUN: &str = r#"pub fn hold() {
    let dir = std::path::PathBuf::from(std::env::var_os("NAGI_TEST_SIGNALS").unwrap());
    std::fs::write(dir.join("entered"), "").unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !dir.join("release").exists() {
        assert!(std::time::Instant::now() < until);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    std::fs::write(dir.join("done"), "").unwrap();
}"#;

#[test]
fn managed_concurrent_run_keeps_an_older_executable_until_actual_exit() {
    let f = Fixture::new();
    f.write("bridge.rs", HOLD_RUN);
    f.write("main.nagi", "@rust(\"native::hold\")\nextern def hold()\ndef main():\n    print(\"held\")\n    hold()\n");
    let signals = f.0.join("running");
    let mut held = managed_command(&f, "main.nagi");
    held.args(["--rust", "bridge.rs"])
        .env("NAGI_TEST_SIGNALS", &signals);
    let mut held = Running::spawn(held, signals);
    held.wait_for("entered");
    let (_, metadata) = f.latest("out");
    let first = f.app("out").join(metadata["executable"].as_str().unwrap());
    let bytes = fs::read(&first).unwrap();
    f.source("main.nagi", "new");
    let latest = native(&bounded_output(managed_command(&f, "main.nagi")));
    assert_eq!(fs::read(&first).unwrap(), bytes);
    held.release();
    success(&held.output());
    assert!(!first.parent().unwrap().exists());
    assert!(
        latest.is_file(),
        "the newer latest must survive an older run completing last"
    );
}

#[test]
fn managed_kernel_lease_preserves_then_reclaims_without_pid_guessing() {
    let f = Fixture::new();
    let first = native(&bounded_output(managed_command(&f, "main.nagi")));
    let pin = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(run_lease(&first))
        .unwrap();
    pin.lock_shared().unwrap();
    let second = native(&bounded_output(managed_command(&f, "main.nagi")));
    assert!(first.is_file());
    assert!(second.is_file());
    drop(pin);
    native(&bounded_output(managed_command(&f, "main.nagi")));
    assert!(!first.parent().unwrap().exists());
    assert!(!run_lease(&first).exists());
}

#[test]
fn native_entry_waiter_observes_tombstone_before_running_user_code() {
    let f = Fixture::new();
    let old = native(&bounded_output(managed_command(&f, "main.nagi")));
    let lease_path = run_lease(&old);
    let mut lease = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&lease_path)
        .unwrap();
    lease.lock().unwrap();

    let signals = f.0.join("retired-entry");
    let mut direct = Command::new(&old);
    direct.env("NAGI_TEST_SIGNALS", &signals);
    direct.env_remove("NAGI_RUN_RETENTION");
    let mut direct = Running::spawn(direct, signals);
    direct.wait_for("lease-waiting");

    use std::io::{Seek, SeekFrom, Write};
    lease.set_len(0).unwrap();
    lease.seek(SeekFrom::Start(0)).unwrap();
    lease.write_all(lease_header(&old, "X").as_bytes()).unwrap();
    lease.sync_all().unwrap();
    drop(lease);

    let output = direct.output();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "retired native code must not run");
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("temporary run generation is retired or its lease is invalid"));
}

#[test]
fn tombstone_journal_recovers_partial_and_already_removed_generation_trees() {
    for remove_whole_tree in [false, true] {
        let f = Fixture::new();
        let old = native(&bounded_output(managed_command(&f, "main.nagi")));
        let generation = old.parent().unwrap().to_owned();
        let journal = run_lease(&old);
        retire_lease(&old);
        if remove_whole_tree {
            fs::remove_dir_all(&generation).unwrap();
        } else {
            // A tombstone is sufficient to resume a partially deleted tree
            // whose dependency metadata has already gone.
            fs::remove_file(generation.join("generation-inputs.json")).unwrap();
        }

        f.source("main.nagi", "replacement");
        let next = native(&bounded_output(managed_command(&f, "main.nagi")));
        assert_eq!(run(&next), "replacement");
        assert!(
            !generation.exists(),
            "recovery must finish the old tree removal"
        );
        assert!(
            !journal.exists(),
            "recovery must remove the external X journal"
        );
    }
}

#[test]
fn missing_inputs_metadata_on_ready_managed_generation_stops_reclamation() {
    let f = Fixture::new();
    let old = native(&bounded_output(managed_command(&f, "main.nagi")));
    let generation = old.parent().unwrap().to_owned();
    fs::remove_file(generation.join("generation-inputs.json")).unwrap();

    f.source("main.nagi", "replacement");
    let next = native(&bounded_output(managed_command(&f, "main.nagi")));
    assert_eq!(run(&next), "replacement");
    assert!(
        generation.is_dir(),
        "unknown managed dependencies must be retained"
    );
    assert!(run_lease(&old).is_file());
}

#[test]
fn direct_native_launch_without_retention_environment_is_pinned_until_exit() {
    let f = Fixture::new();
    f.write(
        "bridge.rs",
        r#"pub fn hold() {
    let Some(signals) = std::env::var_os("NAGI_TEST_SIGNALS") else { return; };
    let dir = std::path::PathBuf::from(signals);
    std::fs::write(dir.join("entered"), "").unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !dir.join("release").exists() {
        assert!(std::time::Instant::now() < until);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    std::fs::write(dir.join("done"), "").unwrap();
}"#,
    );
    f.write(
        "main.nagi",
        "@rust(\"native::hold\")\nextern def hold()\ndef main():\n    hold()\n",
    );
    let old = native(&bounded_output({
        let mut command = managed_command(&f, "main.nagi");
        command.arg("--rust").arg("bridge.rs");
        command
    }));

    let signals = f.0.join("direct-native");
    let mut command = Command::new(&old);
    command.env("NAGI_TEST_SIGNALS", &signals);
    command.env_remove("NAGI_RUN_RETENTION");
    let mut direct = Running::spawn(command, signals);
    direct.wait_for("entered");

    f.source("main.nagi", "new latest");
    native(&bounded_output(managed_command(&f, "main.nagi")));
    assert!(
        old.is_file(),
        "direct native launch must hold its kernel lease"
    );
    direct.release();
    success(&direct.output());

    native(&bounded_output(managed_command(&f, "main.nagi")));
    assert!(!old.parent().unwrap().exists());
    assert!(!run_lease(&old).exists());
}

#[test]
fn killed_parent_keeps_native_lease_through_tls_destructor_and_os_exit() {
    let f = Fixture::new();
    f.write(
        "bridge.rs",
        r#"struct ExitBarrier;
impl Drop for ExitBarrier {
    fn drop(&mut self) {
        let dir = std::path::PathBuf::from(std::env::var_os("NAGI_TEST_SIGNALS").unwrap());
        std::fs::write(dir.join("tls-entered"), "").unwrap();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while !dir.join("release").exists() {
            assert!(std::time::Instant::now() < until);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        std::fs::write(dir.join("tls-done"), "").unwrap();
    }
}
thread_local! { static ON_EXIT: ExitBarrier = ExitBarrier; }
pub fn touch_exit_barrier() { ON_EXIT.with(|_| {}); }"#,
    );
    f.write(
        "main.nagi",
        "@rust(\"native::touch_exit_barrier\")\nextern def touch_exit_barrier()\ndef main():\n    touch_exit_barrier()\n",
    );
    let signals = f.0.join("tls-native");
    let mut command = managed_command(&f, "main.nagi");
    command
        .arg("--rust")
        .arg("bridge.rs")
        .env("NAGI_TEST_SIGNALS", &signals);
    let mut parent = Running::spawn(command, signals);
    parent.wait_for("tls-entered");
    let old = native_path_from_latest(&f, "out");
    let lease_path = run_lease(&old);

    parent.kill_parent();
    let lease = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&lease_path)
        .unwrap();
    assert!(
        matches!(lease.try_lock(), Err(fs::TryLockError::WouldBlock)),
        "native TLS teardown still owns the OS lease after its parent is killed"
    );
    f.source("main.nagi", "new latest");
    native(&bounded_output(managed_command(&f, "main.nagi")));
    assert!(old.is_file(), "a live orphan remains pinned");

    parent.release();
    let until = Instant::now() + DEADLINE;
    while !parent.signals().join("tls-done").exists() {
        assert!(
            Instant::now() < until,
            "native TLS destructor must leave its barrier"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!parent.output().status.success());
    let until = Instant::now() + DEADLINE;
    loop {
        match lease.try_lock() {
            Ok(()) => break,
            Err(fs::TryLockError::WouldBlock) => {
                assert!(
                    Instant::now() < until,
                    "kernel lease must remain until the native process exits"
                );
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("cannot observe native lease release: {error:?}"),
        }
    }
    drop(lease);
    native(&bounded_output(managed_command(&f, "main.nagi")));
    assert!(!old.parent().unwrap().exists());
    assert!(!lease_path.exists());
}

fn native_path_from_latest(f: &Fixture, out: &str) -> PathBuf {
    let (_, metadata) = f.latest(out);
    f.app(out).join(metadata["executable"].as_str().unwrap())
}

#[test]
fn async_native_entry_guard_runs_before_block_on_even_without_environment_opt_in() {
    let f = Fixture::new();
    // This minimal executor is only an entry-order oracle. It does not assert
    // behavior or guarantees of the production Tokio executor.
    f.write(
        "runtime/src/lib.rs",
        r#"use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
struct Noop;
impl Wake for Noop { fn wake(self: Arc<Self>) {} }
pub fn block_on<F: Future>(future: F) -> F::Output {
    if let Some(dir) = std::env::var_os("NAGI_TEST_SIGNALS") {
        std::fs::write(std::path::PathBuf::from(dir).join("block-on"), "").unwrap();
    }
    let waker = Waker::from(Arc::new(Noop));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("ordering fixture does not support pending futures"),
    }
}"#,
    );
    f.write(
        "bridge.rs",
        r#"pub fn user_code() {
    let dir = std::path::PathBuf::from(std::env::var_os("NAGI_TEST_SIGNALS").unwrap());
    std::fs::write(dir.join("user-code"), "").unwrap();
}"#,
    );
    f.write(
        "main.nagi",
        "@rust(\"native::user_code\")\nextern def user_code()\nasync def main():\n    user_code()\n",
    );
    let initial_signals = f.0.join("async-initial");
    fs::create_dir_all(&initial_signals).unwrap();
    let mut initial = managed_command(&f, "main.nagi");
    initial
        .arg("--rust")
        .arg("bridge.rs")
        .env("NAGI_TEST_SIGNALS", &initial_signals);
    let output = bounded_output(initial);
    success(&output);
    let old = native(&output);
    assert!(initial_signals.join("block-on").is_file());
    assert!(initial_signals.join("user-code").is_file());

    let lease_path = run_lease(&old);
    let mut lease = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&lease_path)
        .unwrap();
    lease.lock().unwrap();
    let refused_signals = f.0.join("async-refused");
    let mut direct = Command::new(&old);
    direct.env("NAGI_TEST_SIGNALS", &refused_signals);
    direct.env_remove("NAGI_RUN_RETENTION");
    let mut direct = Running::spawn(direct, refused_signals.clone());
    direct.wait_for("lease-waiting");

    use std::io::{Seek, SeekFrom, Write};
    lease.set_len(0).unwrap();
    lease.seek(SeekFrom::Start(0)).unwrap();
    lease.write_all(lease_header(&old, "X").as_bytes()).unwrap();
    lease.sync_all().unwrap();
    drop(lease);

    let output = direct.output();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!refused_signals.join("block-on").exists());
    assert!(!refused_signals.join("user-code").exists());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("temporary run generation is retired or its lease is invalid"));
}

#[test]
fn synthetic_no_main_native_entry_still_rejects_a_retired_generation() {
    let f = Fixture::new();
    f.write("main.nagi", "def helper():\n    print(\"USER_SENTINEL\")\n");
    let old = native(&bounded_output(managed_command(&f, "main.nagi")));
    assert_eq!(run(&old), "");

    let lease_path = run_lease(&old);
    let mut lease = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&lease_path)
        .unwrap();
    lease.lock().unwrap();
    let signals = f.0.join("no-main-refused");
    let mut direct = Command::new(&old);
    direct.env("NAGI_TEST_SIGNALS", &signals);
    direct.env_remove("NAGI_RUN_RETENTION");
    let mut direct = Running::spawn(direct, signals);
    direct.wait_for("lease-waiting");

    use std::io::{Seek, SeekFrom, Write};
    lease.set_len(0).unwrap();
    lease.seek(SeekFrom::Start(0)).unwrap();
    lease.write_all(lease_header(&old, "X").as_bytes()).unwrap();
    lease.sync_all().unwrap();
    drop(lease);
    let output = direct.output();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("temporary run generation is retired or its lease is invalid"));
}

#[test]
fn managed_parent_termination_does_not_claim_the_native_child_finished() {
    let f = Fixture::new();
    f.write("bridge.rs", HOLD_RUN);
    f.write(
        "main.nagi",
        "@rust(\"native::hold\")\nextern def hold()\ndef main():\n    hold()\n",
    );
    let signals = f.0.join("orphan-native");
    let mut command = managed_command(&f, "main.nagi");
    command
        .args(["--rust", "bridge.rs"])
        .env("NAGI_TEST_SIGNALS", &signals);
    let mut parent = Running::spawn(command, signals);
    parent.wait_for("entered");
    let (_, metadata) = f.latest("out");
    let old = f.app("out").join(metadata["executable"].as_str().unwrap());
    parent.kill_parent();
    f.source("main.nagi", "new");
    let new = native(&bounded_output(managed_command(&f, "main.nagi")));
    assert!(
        old.is_file(),
        "parent termination is not actual native child join"
    );
    assert!(new.is_file());
    parent.release();
    let until = Instant::now() + DEADLINE;
    while !parent.signals().join("done").exists() {
        assert!(
            Instant::now() < until,
            "orphan native child must finish at its barrier"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!parent.output().status.success());
    let lease = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(run_lease(&old))
        .unwrap();
    let until = Instant::now() + DEADLINE;
    loop {
        match lease.try_lock() {
            Ok(()) => break,
            Err(fs::TryLockError::WouldBlock) => {
                assert!(
                    Instant::now() < until,
                    "native OS exit must release its lease"
                );
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("cannot observe native lease release: {error:?}"),
        }
    }
    drop(lease);
    native(&bounded_output(managed_command(&f, "main.nagi")));
    assert!(
        !old.parent().unwrap().exists(),
        "next successful run reclaims a crashed compiler's completed native generation"
    );
    assert!(!run_lease(&old).exists());
}

#[test]
fn managed_cleanup_preserves_native_inputs_inside_an_older_generation() {
    let f = Fixture::new();
    let old = native(&bounded_output(managed_command(&f, "main.nagi")));
    let bridge = old.parent().unwrap().join("bridge.rs");
    fs::write(&bridge, "pub fn value() -> i64 { 42 }\n").unwrap();
    f.write(
        "main.nagi",
        "@rust(\"native::value\")\nextern def value() -> i64\ndef main():\n    print(value())\n",
    );
    let mut command = managed_command(&f, "main.nagi");
    command.arg("--rust").arg(&bridge);
    let output = bounded_output(command);
    let new = native(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
    assert!(old.is_file());
    assert!(
        bridge.is_file(),
        "a native input must not be removed with an old snapshot"
    );
    assert!(new.is_file());
}

#[test]
fn nonlatest_active_run_keeps_its_native_input_after_a_newer_latest_succeeds() {
    let f = Fixture::new();
    let input_owner = native(&bounded_output(managed_command(&f, "main.nagi")));
    let input_generation = input_owner.parent().unwrap().to_owned();
    let bridge = input_generation.join("bridge.rs");
    fs::write(
        &bridge,
        format!("{}\npub fn value() -> i64 {{ 42 }}\n", HOLD_RUN),
    )
    .unwrap();
    f.write(
        "main.nagi",
        "@rust(\"native::value\")\nextern def value() -> i64\n@rust(\"native::hold\")\nextern def hold()\ndef main():\n    print(value())\n    hold()\n",
    );
    let signals = f.0.join("nonlatest-active");
    let mut command = managed_command(&f, "main.nagi");
    command
        .arg("--rust")
        .arg(&bridge)
        .env("NAGI_TEST_SIGNALS", &signals);
    let mut active = Running::spawn(command, signals);
    active.wait_for("entered");
    let active_binary = native_path_from_latest(&f, "out");
    assert_ne!(active_binary, input_owner);

    f.source("main.nagi", "new latest");
    let latest = native(&bounded_output(managed_command(&f, "main.nagi")));
    assert_ne!(latest, active_binary);
    assert!(
        active_binary.is_file(),
        "the active native process remains pinned after it is no longer latest"
    );
    assert!(
        bridge.is_file(),
        "a non-latest active run's input generation must remain intact"
    );

    active.release();
    let output = active.output();
    success(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
    assert!(
        input_generation.exists(),
        "the active dependency owner is collected before its input"
    );
    native(&bounded_output(managed_command(&f, "main.nagi")));
    assert!(!input_generation.exists());
}

#[test]
fn another_apps_successful_snapshot_keeps_a_referenced_generation_until_later_sweep() {
    let f = Fixture::new();
    let input_owner = native(&bounded_output(managed_command(&f, "main.nagi")));
    let input_generation = input_owner.parent().unwrap().to_owned();
    let input_app = app_for(&input_owner)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let input_generation_id = input_generation
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let bridge = input_generation.join("bridge.rs");
    fs::write(&bridge, "pub fn value() -> i64 { 42 }\n").unwrap();
    f.write(
        "other.nagi",
        "@rust(\"native::value\")\nextern def value() -> i64\ndef main():\n    print(value())\n",
    );
    let mut other = managed_command(&f, "other.nagi");
    other.arg("--rust").arg(&bridge);
    let other_first = native(&bounded_output(other));
    let inputs: serde_json::Value = serde_json::from_slice(
        &fs::read(other_first.parent().unwrap().join("generation-inputs.json")).unwrap(),
    )
    .unwrap();
    assert!(inputs["references"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry[0] == input_app && entry[1] == input_generation_id));

    f.source("main.nagi", "new main");
    native(&bounded_output(managed_command(&f, "main.nagi")));
    assert!(
        bridge.is_file(),
        "another app's retained snapshot protects its input"
    );

    f.source("other.nagi", "new other without the native input");
    native(&bounded_output(managed_command(&f, "other.nagi")));
    assert!(
        input_generation.exists(),
        "the previous dependency owner is collected during this sweep"
    );
    native(&bounded_output(managed_command(&f, "main.nagi")));
    assert!(!input_generation.exists());
}

#[test]
fn external_hardlink_input_identity_protects_its_run_lease_until_owner_is_collected() {
    let f = Fixture::new();
    let old = native(&bounded_output(managed_command(&f, "main.nagi")));
    let old_generation = old.parent().unwrap().to_owned();
    let lease = run_lease(&old);
    let alias = f.0.join("external-lease-input.rs");

    // Use a structurally valid external Rust input on the same inode as the
    // lease, then have its native function restore the ready record before
    // cleanup. This exercises identity protection for a hard-linked lease;
    // ordinary source files cannot naturally contain the private lease header.
    fs::write(
        &lease,
        r#"pub fn restore_lease() {
    let path = std::path::PathBuf::from(std::env::var_os("NAGI_TEST_LEASE").unwrap());
    let header = std::env::var("NAGI_TEST_READY_HEADER").unwrap();
    std::fs::write(path, header).unwrap();
}"#,
    )
    .unwrap();
    fs::hard_link(&lease, &alias).unwrap();
    f.write(
        "main.nagi",
        "@rust(\"native::restore_lease\")\nextern def restore_lease()\ndef main():\n    restore_lease()\n",
    );
    let mut command = managed_command(&f, "main.nagi");
    command
        .arg("--rust")
        .arg(&alias)
        .env("NAGI_TEST_LEASE", &lease)
        .env("NAGI_TEST_READY_HEADER", lease_header(&old, "R"));
    let linked_input_run = native(&bounded_output(command));
    assert_ne!(linked_input_run, old);
    assert_eq!(fs::read_to_string(&lease).unwrap(), lease_header(&old, "R"));
    assert!(
        old_generation.exists(),
        "input identity must preserve the journal and its generation"
    );

    f.source("main.nagi", "without linked input");
    native(&bounded_output(managed_command(&f, "main.nagi")));
    assert!(
        old_generation.exists(),
        "the hard-link owner's snapshot is collected before its lease"
    );
    fs::remove_file(&alias).unwrap();
    native(&bounded_output(managed_command(&f, "main.nagi")));
    assert!(!old_generation.exists());
    assert!(!lease.exists());
}

#[test]
fn managed_path_dependency_records_its_known_root_and_preserves_crate_sources() {
    let f = Fixture::new();
    let old = native(&bounded_output(managed_command_at(
        &f,
        "main.nagi",
        "out-a",
    )));
    let old_generation = old.parent().unwrap().to_owned();
    let old_app = app_for(&old)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let old_id = old_generation
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let crate_root = old_generation.join("support-crate");
    fs::create_dir_all(crate_root.join("src")).unwrap();
    fs::write(
        crate_root.join("Cargo.toml"),
        "[package]\nname='support_crate'\nversion='0.1.0'\nedition='2021'\n[lib]\npath='src/lib.rs'\n[workspace]\n",
    )
    .unwrap();
    fs::write(
        crate_root.join("src/lib.rs"),
        "pub fn value() -> i64 { 73 }\n",
    )
    .unwrap();

    f.write(
        "consumer/main.nagi",
        "@rust(\"native::value\")\nextern def value() -> i64\ndef main():\n    print(value())\n",
    );
    f.write(
        "consumer/native.rs",
        "pub fn value() -> i64 { support_crate::value() }\n",
    );
    let crate_path = serde_json::to_string(&crate_root.to_string_lossy()).unwrap();
    f.write(
        "consumer/nagi.toml",
        &format!(
            "entry='main.nagi'\n[rust]\nfile='native.rs'\n[rust.dependencies]\nsupport_crate={{path={crate_path}}}\n"
        ),
    );
    let consumer = native(&bounded_output(managed_project_command(
        &f, "consumer", "out-a",
    )));
    assert_eq!(run(&consumer), "73");
    let input_snapshot: serde_json::Value = serde_json::from_slice(
        &fs::read(consumer.parent().unwrap().join("generation-inputs.json")).unwrap(),
    )
    .unwrap();
    assert!(
        input_snapshot["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry[0] == old_app && entry[1] == old_id),
        "path dependency manifest and crate root inside an older generation must be recorded"
    );
    // Unix offers a stable independent inode oracle. Windows checks the same
    // root reference and observable native/bytes behavior below; its by-handle
    // MetadataExt IDs are nightly-only. Production uses the existing Win32
    // identity boundary, not a public API or a test-only nightly feature.
    #[cfg(unix)]
    {
        let identities = input_snapshot["identities"].as_array().unwrap();
        let manifest = crate_root.join("Cargo.toml");
        let identity = serde_json::json!(test_file_identity(&manifest));
        assert!(
            identities.contains(&identity),
            "the explicitly known crate manifest identity must be recorded"
        );
    }

    f.source("main.nagi", "replace old owner latest");
    native(&bounded_output(managed_command_at(
        &f,
        "main.nagi",
        "out-a",
    )));
    assert!(
        crate_root.join("Cargo.toml").is_file(),
        "the managed dependency owner cannot reclaim its path crate"
    );
    assert!(
        crate_root.join("src/lib.rs").is_file(),
        "the managed dependency owner cannot remove the crate source"
    );
}

#[test]
fn another_output_root_preserves_an_exported_managed_generation() {
    let f = Fixture::new();
    let old = native(&bounded_output(managed_command_at(
        &f,
        "main.nagi",
        "out-a",
    )));
    let old_generation = old.parent().unwrap().to_owned();
    let bridge = old_generation.join("bridge.rs");
    let saved_low = old_generation.join("generated.low");
    fs::write(&bridge, "pub fn value() -> i64 { 84 }\n").unwrap();
    fs::write(
        &saved_low,
        "@rust(\"native::value\")\nextern fn value() -> i64;\nfn main() -> unit {\n    print(value());\n}\n",
    )
    .unwrap();

    let mut external = f.cli("run", saved_low.to_str().unwrap(), "out-b");
    external
        .arg("--rust")
        .arg(&bridge)
        .env("NAGI_RUN_RETENTION", "latest");
    let consumer = native(&bounded_output(external));
    assert_eq!(run(&consumer), "84");

    f.source("main.nagi", "replace exported owner latest");
    native(&bounded_output(managed_command_at(
        &f,
        "main.nagi",
        "out-a",
    )));
    assert!(
        old_generation.is_dir(),
        "an export consumed from another output root must keep its owner immutable"
    );
    assert!(bridge.is_file());
    assert!(saved_low.is_file());
}

#[test]
fn managed_older_completion_preserves_newer_latest_native_inputs() {
    let f = Fixture::new();
    let old = native(&bounded_output(managed_command(&f, "main.nagi")));
    let input = old.parent().unwrap().join("bridge.rs");
    fs::write(&input, "pub fn value() -> i64 { 42 }\n").unwrap();
    f.write("hold.rs", HOLD_RUN);
    f.write(
        "main.nagi",
        "@rust(\"native::hold\")\nextern def hold()\ndef main():\n    hold()\n",
    );
    let signals = f.0.join("older-finish");
    let mut command = managed_command(&f, "main.nagi");
    command
        .arg("--rust")
        .arg("hold.rs")
        .env("NAGI_TEST_SIGNALS", &signals);
    let mut held = Running::spawn(command, signals);
    held.wait_for("entered");
    f.write(
        "main.nagi",
        "@rust(\"native::value\")\nextern def value() -> i64\ndef main():\n    print(value())\n",
    );
    let mut newer = managed_command(&f, "main.nagi");
    newer.arg("--rust").arg(&input);
    let output = bounded_output(newer);
    let latest = native(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
    assert!(input.is_file(), "newer completion preserves its input");
    held.release();
    success(&held.output());
    assert!(latest.is_file());
    assert!(
        input.is_file(),
        "an older completion must preserve the newer latest's native input"
    );
    assert!(old.is_file());
}

#[test]
fn managed_older_success_after_newer_failure_keeps_last_good() {
    let f = Fixture::new();
    f.write("hold.rs", HOLD_RUN);
    f.write(
        "main.nagi",
        "@rust(\"native::hold\")\nextern def hold()\ndef main():\n    hold()\n",
    );
    let signals = f.0.join("older-success");
    let mut command = managed_command(&f, "main.nagi");
    command
        .arg("--rust")
        .arg("hold.rs")
        .env("NAGI_TEST_SIGNALS", &signals);
    let mut held = Running::spawn(command, signals);
    held.wait_for("entered");
    let (_, metadata) = f.latest("out");
    let good = f.app("out").join(metadata["executable"].as_str().unwrap());
    f.write("fail.rs", "pub fn fail() { std::process::exit(7); }\n");
    f.write(
        "main.nagi",
        "@rust(\"native::fail\")\nextern def fail()\ndef main():\n    fail()\n",
    );
    let mut command = managed_command(&f, "main.nagi");
    command.args(["--rust", "fail.rs"]);
    let failure = bounded_output(command);
    assert!(!failure.status.success());
    assert!(String::from_utf8_lossy(&failure.stderr).contains("program exited:"));
    assert!(good.is_file());
    held.release();
    success(&held.output());
    assert!(
        good.is_file(),
        "an older success must not discard the last good run after the latest run failed"
    );
    let (_, failed) = f.latest("out");
    assert!(f
        .app("out")
        .join(failed["executable"].as_str().unwrap())
        .is_file());
}

#[test]
fn managed_oversized_record_does_not_stop_normal_reclamation() {
    let f = Fixture::new();
    let unknown = native(&bounded_output(managed_command(&f, "main.nagi")));
    fs::write(run_lease(&unknown), " ".repeat(4097)).unwrap();
    let normal_output = bounded_output(managed_command(&f, "main.nagi"));
    let normal = native(&normal_output);
    let final_output = bounded_output(managed_command(&f, "main.nagi"));
    native(&final_output);
    assert!(unknown.is_file());
    assert!(
        !normal.parent().unwrap().exists(),
        "an unknown lease must not prevent reclamation of normal completed generations"
    );
    for output in [normal_output, final_output] {
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("cleanup warning"),
            "unknown individual records are held rather than aborting the sweep"
        );
    }
}

#[test]
fn managed_saved_low_runs_use_the_same_retention_contract() {
    let f = Fixture::new();
    success(&bounded_output(f.cli("lower", "main.nagi", "lowered")));
    fs::copy(f.0.join("lowered/generated.low"), f.0.join("saved.low")).unwrap();
    let old = native(&bounded_output(managed_command(&f, "saved.low")));
    let new = native(&bounded_output(managed_command(&f, "saved.low")));
    assert!(!old.parent().unwrap().exists());
    assert_eq!(run(&new), "old");
}

#[test]
fn managed_cleanup_preserves_unknown_or_oversized_retention_metadata() {
    for marker in ["{}".to_owned(), " ".repeat(65537)] {
        let f = Fixture::new();
        let old = native(&bounded_output(managed_command(&f, "main.nagi")));
        fs::write(run_lease(&old), marker).unwrap();
        native(&bounded_output(managed_command(&f, "main.nagi")));
        assert!(old.is_file());
    }
}

#[cfg(unix)]
#[test]
fn managed_cleanup_skips_links_and_does_not_touch_other_apps_or_staging() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let first = native(&bounded_output(managed_command(&f, "main.nagi")));
    let app = f.app("out");
    let external = f.0.join("user-data");
    fs::create_dir(&external).unwrap();
    fs::write(external.join("keep"), "user data").unwrap();
    symlink(&external, first.parent().unwrap().join("unexpected-link")).unwrap();
    let staging = app.join("generations/.staging-g-1-2-3");
    fs::create_dir(&staging).unwrap();
    fs::write(staging.join("evidence"), "failed build").unwrap();
    f.source("other.nagi", "other app");
    let other = native(&bounded_output(managed_command(&f, "other.nagi")));
    native(&bounded_output(managed_command(&f, "main.nagi")));
    assert!(first.is_file());
    assert_eq!(
        fs::read_to_string(external.join("keep")).unwrap(),
        "user data"
    );
    assert!(other.is_file());
    assert!(staging.join("evidence").is_file());
}

#[cfg(windows)]
#[test]
fn managed_cleanup_preserves_a_standard_junction_reparse_point() {
    use std::os::windows::fs::MetadataExt;

    let f = Fixture::new();
    let old = native(&bounded_output(managed_command(&f, "main.nagi")));
    let generation = old.parent().unwrap().to_owned();
    let target = f.0.join("junction-target");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("keep"), "caller data").unwrap();
    let junction = generation.join("caller-junction");
    let result = Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(&junction)
        .arg(&target)
        .output()
        .expect("Windows cmd.exe must be available");
    assert!(
        result.status.success(),
        "mklink /J must create the ordinary reparse-point fixture: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let metadata = fs::symlink_metadata(&junction).unwrap();
    assert_ne!(
        metadata.file_attributes() & 0x400,
        0,
        "the fixture must be an actual Windows reparse point"
    );

    f.source("main.nagi", "replacement");
    native(&bounded_output(managed_command(&f, "main.nagi")));
    assert!(
        generation.is_dir(),
        "cleanup must leave a generation containing a junction"
    );
    assert_eq!(
        fs::read_to_string(target.join("keep")).unwrap(),
        "caller data"
    );
    assert!(junction.exists());
}

#[test]
fn cargo_cache_keeps_a_preexisting_default_package_binary_unchanged() {
    let f = Fixture::new();
    f.build();
    let p = package(&f.0.join("out"));
    let old_cache =
        f.0.join("cache/release")
            .join(format!("{p}{}", std::env::consts::EXE_SUFFIX));
    fs::write(&old_cache, b"caller-owned default package artifact").unwrap();
    f.source("main.nagi", "new");
    let native = f.build();
    assert_eq!(
        fs::read(old_cache).unwrap(),
        b"caller-owned default package artifact"
    );
    assert_eq!(run(&native), "new");
}

#[test]
fn same_and_different_apps_with_equivalent_output_paths_wait_for_one_output_writer() {
    for source in ["main.nagi", "second.nagi"] {
        let f = Fixture::new();
        let wrapper = f.wrapper();
        let mut first = Running::wrapper(
            f.wrapped(&wrapper, "run", "main.nagi", "out", "first", "after"),
            f.0.join("first"),
        );
        first.wait_for("compiled");
        let first_low = fs::read(f.0.join("out/generated.low")).unwrap();
        f.source(source, "second");
        let mut second = Running::wrapper(
            f.wrapped(&wrapper, "run", source, "out/../out", "second", "normal"),
            f.0.join("second"),
        );
        // The first writer is positively held after Cargo releases its target lock.
        // An entered marker is proof that a competing Nagi writer escaped the out lock.
        os_writer_is_held(&f, "out");
        second.wait_for("waiting");
        assert!(
            !second.signals().join("entered").exists(),
            "second Cargo entered while first generation was unpublished"
        );
        assert_eq!(fs::read(f.0.join("out/generated.low")).unwrap(), first_low);
        first.release();
        let first_output = first.output();
        success(&first_output);
        assert_eq!(String::from_utf8_lossy(&first_output.stdout).trim(), "old");
        second.wait_for("entered");
        let output = second.output();
        success(&output);
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "second");
    }
}

#[test]
fn writing_check_lower_and_cost_wait_while_build_owns_the_output_lock() {
    for (verb, cost) in [("check", false), ("lower", false), ("check", true)] {
        let f = Fixture::new();
        f.source("second.nagi", "second");
        let wrapper = f.wrapper();
        let mut first = Running::wrapper(
            f.wrapped(&wrapper, "build", "main.nagi", "out", "first", "after"),
            f.0.join("first"),
        );
        first.wait_for("compiled");
        let before = fs::read(f.0.join("out/generated.low")).unwrap();
        let mut command = f.cli(verb, "second.nagi", "out");
        if cost {
            command.arg("--cost-report");
        }
        let mut second = Running::spawn(command, f.0.join("unused"));
        os_writer_is_held(&f, "out");
        second.wait_for("waiting");
        assert!(second.child.as_mut().unwrap().try_wait().unwrap().is_none());
        assert_eq!(fs::read(f.0.join("out/generated.low")).unwrap(), before);
        assert!(!f.0.join("out/cost-report.json").exists());
        first.release();
        success(&first.output());
        success(&second.output());
        assert!(fs::read_to_string(f.0.join("out/generated.low"))
            .unwrap()
            .contains("second"));
        assert_eq!(f.0.join("out/cost-report.json").exists(), cost);
    }
}

#[test]
fn lock_is_permanent_and_input_protection_is_rechecked_after_waiting() {
    let f = Fixture::new();
    fs::create_dir_all(f.0.join("out")).unwrap();
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(f.0.join("out/.nagi-write.lock"))
        .unwrap();
    lock.lock().unwrap();
    let mut writer = Running::spawn(f.cli("check", "main.nagi", "out"), f.0.join("unused"));
    writer.wait_for("waiting");
    assert!(writer.child.as_mut().unwrap().try_wait().unwrap().is_none());
    assert!(!f.0.join("out/generated.low").exists());
    // The initial non-writing preflight succeeded; the output changes while waiting.
    fs::hard_link(f.0.join("main.nagi"), f.0.join("out/generated.low")).unwrap();
    let before = fs::read(f.0.join("main.nagi")).unwrap();
    drop(lock);
    let output = writer.output();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot overwrite input file"));
    assert_eq!(fs::read(f.0.join("main.nagi")).unwrap(), before);
    assert!(f.0.join("out/.nagi-write.lock").is_file());
}

// This fixture needs a filesystem that accepts invalid UTF-8 names. macOS APFS
// rejects its directory creation with EILSEQ before nagic can be invoked.
// All platforms still verify their raw OS path units in the Unicode fixture.
#[cfg(target_os = "linux")]
#[test]
fn source_snapshot_retains_raw_non_utf8_canonical_path_without_rejecting_old_accepted_input() {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    let f = Fixture::new();
    let entry =
        f.0.join(std::ffi::OsString::from_vec(b"entry-\xff".to_vec()));
    fs::create_dir(&entry).unwrap();
    fs::write(entry.join("main.nagi"), "def main():\n    print(42)\n").unwrap();
    let out = f.0.join("out");
    let mut command = f.cli("run", "main.nagi", out.to_str().unwrap());
    command.current_dir(&entry);
    let output = bounded_output(command);
    let native = native(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
    let metadata = validate_latest(&f, "out", &native);
    let snapshot: serde_json::Value = serde_json::from_slice(
        &fs::read(f.app("out").join(metadata["sources"].as_str().unwrap())).unwrap(),
    )
    .unwrap();
    let file = &snapshot["files"][0];
    assert_eq!(file["path_os"]["encoding"], "unix_bytes");
    let raw: Vec<u8> = file["path_os"]["units"]
        .as_array()
        .unwrap()
        .iter()
        .map(|unit| u8::try_from(unit.as_u64().unwrap()).unwrap())
        .collect();
    assert_eq!(
        raw,
        fs::canonicalize(entry.join("main.nagi"))
            .unwrap()
            .as_os_str()
            .as_bytes()
    );
}

#[test]
fn latest_metadata_used_as_an_input_cannot_be_overwritten_by_rebuild() {
    let f = Fixture::new();
    let old = f.build();
    let (before, _) = f.latest("out");
    let package = package(&f.0.join("out"));
    f.write(
        "main.nagi",
        &format!(
            "def main():\n    print(include_text(\"out/.nagi/apps/{package}/latest.json\"))\n"
        ),
    );
    let output = bounded_output(f.cli("build", "main.nagi", "out"));
    assert!(!output.status.success(), "metadata input must be protected");
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot overwrite input file"));
    assert_eq!(f.latest("out").0, before);
    assert_eq!(run(&old), "old");
}

#[test]
fn nonwriting_low_check_and_lower_do_not_wait_for_the_output_writer_lock() {
    let f = Fixture::new();
    f.write("main.low", "fn main() { print(42); }\n");
    fs::create_dir_all(f.0.join("out")).unwrap();
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(f.0.join("out/.nagi-write.lock"))
        .unwrap();
    lock.lock().unwrap();
    for verb in ["check", "lower"] {
        let output = bounded_output(f.cli(verb, "main.low", "out"));
        success(&output);
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("waiting for output lock:"));
        assert!(!f.0.join("out/generated.low").exists());
    }
}

#[test]
fn published_manifest_runtime_reference_resolves_from_the_published_location() {
    let f = Fixture::new();
    let native = f.build();
    let metadata = validate_latest(&f, "out", &native);
    let manifest_path = f.app("out").join(metadata["manifest"].as_str().unwrap());
    let manifest: toml::Value =
        toml::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    let runtime = manifest["dependencies"]["nagi-runtime"]["path"]
        .as_str()
        .unwrap();
    assert_eq!(
        fs::canonicalize(manifest_path.parent().unwrap().join(runtime)).unwrap(),
        fs::canonicalize(f.0.join("runtime")).unwrap()
    );
}

#[test]
fn compatibility_outputs_cannot_alias_previous_latest_metadata() {
    for name in ["generated.low", "src/main.rs", "Cargo.toml"] {
        let f = Fixture::new();
        let old = f.build();
        let app = f.app("out");
        let latest = app.join("latest.json");
        let before = fs::read(&latest).unwrap();
        let output = f.0.join("out").join(name);
        fs::remove_file(&output).unwrap();
        fs::hard_link(&latest, &output).unwrap();
        f.source("main.nagi", "new");
        let result = bounded_output(f.cli("build", "main.nagi", "out"));
        assert!(
            !result.status.success(),
            "{name} alias must be refused before writes"
        );
        assert!(String::from_utf8_lossy(&result.stderr).contains("output files must be distinct"));
        assert!(
            fs::read(&latest).unwrap() == before,
            "old latest bytes must survive {name}"
        );
        assert_eq!(run(&old), "old");
    }
}

#[cfg(unix)]
#[test]
fn long_basename_accepted_by_the_previous_cache_scheme_uses_a_short_generation_bin() {
    let f = Fixture::new();
    let source = format!("{}.nagi", "x".repeat(200));
    f.source(&source, "long basename");
    let native = native(&bounded_output(f.cli("build", &source, "out")));
    let metadata = validate_latest(&f, "out", &native);
    assert_eq!(run(&native), "long basename");
    assert_eq!(
        metadata["app_id"].as_str().unwrap(),
        package(&f.0.join("out"))
    );
    assert!(
        native.file_name().unwrap().len() < 100,
        "bin identity need not repeat the full source stem"
    );
}

fn writing_command_cannot_alias_latest(verb: &str, name: &str, cost: bool) {
    let f = Fixture::new();
    let old = f.build();
    let latest = f.app("out").join("latest.json");
    let before = fs::read(&latest).unwrap();
    let target = f.0.join("out").join(name);
    if target.exists() {
        fs::remove_file(&target).unwrap();
    }
    fs::hard_link(&latest, target).unwrap();
    f.source("main.nagi", "new");
    let mut command = f.cli(verb, "main.nagi", "out");
    if cost {
        command.arg("--cost-report");
    }
    let output = bounded_output(command);
    assert!(
        !output.status.success(),
        "{verb}/{name} must not replace success metadata"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("output files must be distinct"));
    assert!(fs::read(&latest).unwrap() == before);
    assert_eq!(run(&old), "old");
}
#[test]
fn writing_high_check_cannot_alias_previous_latest() {
    writing_command_cannot_alias_latest("check", "generated.low", false);
}
#[test]
fn writing_high_lower_cannot_alias_previous_latest() {
    writing_command_cannot_alias_latest("lower", "generated.low", false);
}
#[test]
fn writing_cost_report_cannot_alias_previous_latest() {
    for verb in ["check", "lower"] {
        writing_command_cannot_alias_latest(verb, "cost-report.json", true);
    }
}

#[test]
fn invalid_input_does_not_create_output_or_its_lock() {
    for source in ["def main(:\n", "def main():\n    print(undefined)\n"] {
        let f = Fixture::new();
        f.write("main.nagi", source);
        let output = bounded_output(f.cli("build", "main.nagi", "out"));
        assert!(!output.status.success());
        assert!(!f.0.join("out").exists());
    }
}

#[test]
fn cargo_failure_retains_latest_and_inherited_lockfile() {
    let f = Fixture::new();
    let old = f.build();
    let (latest, _) = f.latest("out");
    let lock = f.0.join("out/Cargo.lock");
    let mut marked = fs::read(&lock).unwrap();
    marked.extend_from_slice(b"\n# caller-owned marker\n");
    fs::write(&lock, &marked).unwrap();
    let wrapper = f.wrapper();
    f.source("main.nagi", "new");
    let output = bounded_output(f.wrapped(&wrapper, "build", "main.nagi", "out", "failed", "fail"));
    assert!(!output.status.success());
    assert_eq!(f.latest("out").0, latest);
    assert_eq!(fs::read(lock).unwrap(), marked);
    assert_eq!(run(&old), "old");
    let manifest_path = fs::read_to_string(f.0.join("failed/manifest")).unwrap();
    let staged = PathBuf::from(manifest_path);
    assert_eq!(
        fs::read(staged.parent().unwrap().join("Cargo.lock")).unwrap(),
        marked
    );
    // Retry successfully: Cargo updates its staging lock, then Nagi copies it
    // back without losing an unchanged caller-owned comment.
    let retry = f.build();
    assert_eq!(run(&retry), "new");
    assert_eq!(fs::read(f.0.join("out/Cargo.lock")).unwrap(), marked);
    let retry_metadata = validate_latest(&f, "out", &retry);
    assert_eq!(
        fs::read(f.app("out").join(retry_metadata["lock"].as_str().unwrap())).unwrap(),
        marked
    );
}

#[test]
fn projection_failure_keeps_old_latest_and_reports_partial_projection() {
    let f = Fixture::new();
    let old = f.build();
    let (latest, _) = f.latest("out");
    let wrapper = f.wrapper();
    f.source("main.nagi", "new");
    let output = bounded_output(f.wrapped(
        &wrapper,
        "run",
        "main.nagi",
        "out",
        "fault",
        "projection-fault",
    ));
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(f.latest("out").0, latest);
    assert_eq!(run(&old), "old");
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("projection") && error.contains("partial"),
        "{error}"
    );
}

#[test]
fn latest_temp_failure_keeps_old_metadata_and_does_not_run_new_generation() {
    let f = Fixture::new();
    let old = f.build();
    let (latest, _) = f.latest("out");
    let wrapper = f.wrapper();
    f.source("main.nagi", "new");
    let output =
        bounded_output(f.wrapped(&wrapper, "run", "main.nagi", "out", "fault", "latest-fault"));
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(f.latest("out").0, latest);
    assert_eq!(run(&old), "old");
}

#[test]
fn orphan_cargo_uses_an_immutable_snapshot_and_cannot_replace_next_generation() {
    let f = Fixture::new();
    let wrapper = f.wrapper();
    let mut first = Running::wrapper(
        f.wrapped(&wrapper, "build", "main.nagi", "out", "orphan", "before"),
        f.0.join("orphan"),
    );
    first.wait_for("entered");
    let manifest = PathBuf::from(fs::read_to_string(f.0.join("orphan/manifest")).unwrap());
    let snapshot = fs::read(manifest.parent().unwrap().join("src/main.rs")).unwrap();
    let old_manifest: toml::Value =
        toml::from_str(&fs::read_to_string(&manifest).unwrap()).unwrap();
    let old_bin = old_manifest["bin"][0]["name"].as_str().unwrap();
    let args: Vec<String> =
        serde_json::from_slice(&fs::read(f.0.join("orphan/args.json")).unwrap()).unwrap();
    let selected = args
        .iter()
        .position(|arg| arg == "--bin")
        .expect("generation build must explicitly select a bin");
    assert_eq!(args[selected + 1], old_bin);
    // The wrapper is now an OS-owned grandchild. done/status proves it finished;
    // the test cannot reap an adopted process without a platform subreaper.
    // Its own direct Cargo child is polled, killed on timeout, and waited.
    first.kill_parent();
    f.source("main.nagi", "new");
    let next = f.build();
    let bytes = fs::read(&next).unwrap();
    assert_ne!(old_bin, next.file_stem().unwrap().to_str().unwrap());
    let next_cache = f.0.join("cache/release").join(next.file_name().unwrap());
    let cache_bytes = fs::read(&next_cache).unwrap();
    assert!(
        fs::read(manifest.parent().unwrap().join("src/main.rs")).unwrap() == snapshot,
        "an orphan Cargo must not read mutable compatibility output"
    );
    first.release();
    let until = Instant::now() + DEADLINE;
    while !f.0.join("orphan/done").exists() {
        assert!(Instant::now() < until, "orphan Cargo did not finish");
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(fs::read_to_string(f.0.join("orphan/done")).unwrap(), "0");
    assert!(f.0.join("orphan/compiled").is_file());
    assert!(fs::read(&next).unwrap() == bytes);
    assert!(fs::read(next_cache).unwrap() == cache_bytes);
    assert_eq!(
        run(&f
            .0
            .join("cache/release")
            .join(format!("{old_bin}{}", std::env::consts::EXE_SUFFIX))),
        "old"
    );
    assert_eq!(run(&next), "new");
    assert_eq!(
        validate_latest(&f, "out", &next)["executable"]
            .as_str()
            .unwrap(),
        next.strip_prefix(fs::canonicalize(f.app("out")).unwrap())
            .unwrap()
            .to_str()
            .unwrap()
            .replace('\\', "/")
    );
}

struct LatestReader {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<thread::JoinHandle<Result<(), String>>>,
    observed: std::sync::mpsc::Receiver<String>,
}
impl LatestReader {
    fn start(app: PathBuf) -> Self {
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let reader_stop = stop.clone();
        let (send, observed) = std::sync::mpsc::sync_channel(1);
        let handle = thread::spawn(move || -> Result<(), String> {
            while !reader_stop.load(Ordering::Acquire) {
                let bytes = fs::read(app.join("latest.json")).map_err(|e| e.to_string())?;
                let value: serde_json::Value =
                    serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                let generation = value["generation"].as_str().ok_or("missing generation")?;
                for field in [
                    "executable",
                    "manifest",
                    "low",
                    "rust",
                    "sources",
                    "provenance",
                    "lock",
                ] {
                    let relative = value[field]
                        .as_str()
                        .ok_or_else(|| format!("missing {field}"))?;
                    let path = app.join(relative);
                    if !path.is_file()
                        || !path.starts_with(app.join("generations").join(generation))
                    {
                        return Err(format!("incomplete published {field}: {}", path.display()));
                    }
                }
                let _ = send.try_send(generation.to_string());
                thread::yield_now();
            }
            Ok(())
        });
        Self {
            stop,
            thread: Some(handle),
            observed,
        }
    }
    fn acknowledge(&self, generation: &str) {
        let until = Instant::now() + DEADLINE;
        loop {
            let remaining = until.saturating_duration_since(Instant::now());
            assert!(
                !remaining.is_zero(),
                "reader did not acknowledge {generation}"
            );
            let observed = self
                .observed
                .recv_timeout(remaining)
                .expect("reader must publish a complete-generation acknowledgement");
            if observed == generation {
                return;
            }
        }
    }
    fn finish(mut self) {
        self.stop.store(true, Ordering::Release);
        self.thread.take().unwrap().join().unwrap().unwrap();
    }
}
impl Drop for LatestReader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}
#[test]
fn parallel_latest_readers_observe_only_complete_published_generations() {
    let f = Fixture::new();
    let first = f.build();
    let first_metadata = validate_latest(&f, "out", &first);
    let reader = LatestReader::start(f.app("out"));
    reader.acknowledge(first_metadata["generation"].as_str().unwrap());
    for value in ["two", "three", "four"] {
        f.source("main.nagi", value);
        let native = f.build();
        let metadata = validate_latest(&f, "out", &native);
        reader.acknowledge(metadata["generation"].as_str().unwrap());
    }
    reader.finish();
}

#[test]
fn user_low_snapshot_comes_from_this_input_instead_of_a_stale_high_projection() {
    let f = Fixture::new();
    let low = "fn main() { print(\"current Low input\"); }\n";
    f.write("main.low", low);
    f.write("out/generated.low", "stale High generated Low sentinel");
    let native = native(&bounded_output(f.cli("build", "main.low", "out")));
    let metadata = validate_latest(&f, "out", &native);
    assert_eq!(run(&native), "current Low input");
    assert_eq!(
        fs::read_to_string(f.app("out").join(metadata["low"].as_str().unwrap())).unwrap(),
        low
    );
    assert_eq!(
        fs::read_to_string(f.0.join("out/generated.low")).unwrap(),
        "stale High generated Low sentinel"
    );
    let sources: serde_json::Value = serde_json::from_slice(
        &fs::read(f.app("out").join(metadata["sources"].as_str().unwrap())).unwrap(),
    )
    .unwrap();
    assert!(sources["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|file| file["text"] == low && file["kind"] == "user_low"));
}

#[cfg(windows)]
#[test]
fn latest_rename_failure_preserves_old_success_and_recovers_after_handle_close() {
    use std::os::windows::fs::OpenOptionsExt;
    let f = Fixture::new();
    let old = f.build();
    let (latest, old_metadata) = f.latest("out");
    let path = f.app("out").join("latest.json");
    let handle = fs::OpenOptions::new()
        .read(true)
        .share_mode(0x1)
        .open(&path)
        .unwrap();
    f.source("main.nagi", "new");
    let output = bounded_output(f.cli("run", "main.nagi", "out"));
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr)
        .lines()
        .any(|line| line.starts_with("native: ")));
    let published: Vec<_> = fs::read_dir(f.app("out").join("generations"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|generation| generation != old_metadata["generation"].as_str().unwrap())
        .collect();
    assert_eq!(
        published.len(),
        1,
        "failure occurs after publishing the new immutable generation"
    );
    let temp = f.app("out").join(format!(".latest-{}.tmp", published[0]));
    assert!(
        temp.is_file(),
        "replacement failure must retain the written metadata temp file"
    );
    assert_eq!(fs::read(&path).unwrap(), latest);
    assert_eq!(run(&old), "old");
    drop(handle);
    let new = f.build();
    assert_ne!(new, old);
    assert_eq!(run(&new), "new");
}

#[test]
fn run_releases_output_lock_before_waiting_for_the_selected_executable() {
    let f = Fixture::new();
    f.source("second.nagi", "second");
    f.write(
        "bridge.rs",
        r#"pub fn hold() {
        let dir = std::path::PathBuf::from(std::env::var_os("NAGI_TEST_SIGNALS").unwrap());
        std::fs::write(dir.join("entered"), "").unwrap();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while !dir.join("release").exists() {
            assert!(std::time::Instant::now() < until);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }"#,
    );
    f.write("main.nagi", "@rust(\"native::hold\")\nextern def hold()\ndef main():\n    print(\"selected old\")\n    hold()\n");
    let signals = f.0.join("running");
    fs::create_dir_all(&signals).unwrap();
    let mut first = f.cli("run", "main.nagi", "out");
    first
        .args(["--rust", "bridge.rs"])
        .env("NAGI_TEST_SIGNALS", &signals);
    let mut running = Running::spawn(first, signals);
    running.wait_for("entered");
    let next = Running::spawn(f.cli("run", "second.nagi", "out"), f.0.join("unused"));
    let output = next.output();
    success(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "second");
    running.release();
    let output = running.output();
    success(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "selected old"
    );
}

// std-only wrapper works on Windows and Unix. All held children have a deadline
// and a release-on-drop signal, including the deliberately orphaned Cargo.
const CARGO_WRAPPER: &str = r#"use std::{env, fs, path::PathBuf, process::{Command, exit}, thread, time::{Duration, Instant}};
fn barrier(signals: &std::path::Path) {
    let until = Instant::now() + Duration::from_secs(60);
    while !signals.join("release").exists() {
        if Instant::now() > until { exit(99); } thread::sleep(Duration::from_millis(10));
    }
}
fn main() {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let manifest = PathBuf::from(&args[args.iter().position(|a| a == "--manifest-path").unwrap()+1]);
    let signals = PathBuf::from(env::var_os("NAGI_TEST_SIGNALS").unwrap());
    let mode = env::var("NAGI_TEST_MODE").unwrap();
    fs::write(signals.join("manifest"), manifest.to_str().unwrap()).unwrap();
    // JSON quote without an external crate: CLI-generated bin names are ASCII,
    // while paths require escaping backslashes/quotes for Windows and spaces.
    let encoded: Vec<_> = args.iter().map(|arg| format!("\"{}\"", arg.to_str().unwrap().replace('\\', "\\\\").replace('\"', "\\\""))).collect();
    fs::write(signals.join("args.json"), format!("[{}]", encoded.join(","))).unwrap();
    fs::write(signals.join("entered"), "").unwrap();
    if mode == "before" { barrier(&signals); }
    if mode == "fail" { fs::write(signals.join("done"), "42").unwrap(); exit(42); }
    let mut command = Command::new(env::var_os("NAGI_TEST_REAL_CARGO").unwrap());
    command.args(&args);
    // An orphan Cargo must not write JSON to its dead Nagi parent's pipe.
    if mode == "before" {
        command.stdout(fs::File::create(signals.join("cargo.stdout")).unwrap())
            .stderr(fs::File::create(signals.join("cargo.stderr")).unwrap());
    }
    let mut cargo = command.spawn().unwrap();
    let until = Instant::now() + Duration::from_secs(45);
    let status = loop {
        if let Some(status) = cargo.try_wait().unwrap() { break status; }
        if Instant::now() > until {
            let _ = cargo.kill(); let _ = cargo.wait();
            fs::write(signals.join("done"), "timeout").unwrap(); exit(98);
        }
        thread::sleep(Duration::from_millis(10));
    };
    if status.success() {
        fs::write(signals.join("compiled"), "").unwrap();
        if mode == "after" { barrier(&signals); }
        if mode == "projection-fault" {
            let out = PathBuf::from(env::var_os("NAGI_TEST_OUT").unwrap());
            let path = out.join("src/main.rs"); fs::remove_file(&path).unwrap(); fs::create_dir(path).unwrap();
        }
        if mode == "latest-fault" {
            // ADR007: staging and published generation share the generations parent.
            // This keeps relative runtime paths valid after publication.
            // The generation name is the staging directory name without .staging-.
            let staging = manifest.parent().unwrap(); let app = staging.parent().unwrap().parent().unwrap();
            let generation = staging.file_name().unwrap().to_str().unwrap().strip_prefix(".staging-").unwrap();
            fs::create_dir(app.join(format!(".latest-{generation}.tmp"))).unwrap();
        }
    }
    fs::write(signals.join("done"), status.code().unwrap_or(1).to_string()).unwrap();
    exit(status.code().unwrap_or(1));
}"#;
