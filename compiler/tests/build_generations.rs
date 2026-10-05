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

#[cfg(unix)]
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
