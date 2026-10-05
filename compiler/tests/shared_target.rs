use std::{
    fs,
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        Self::new_at(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        )
    }

    // Inject repeated clock ticks without depending on host timer resolution.
    fn new_at(timestamp: u128) -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi shared cache 凪 {} {} {}",
            std::process::id(),
            timestamp,
            FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        // Clock resolution is not a uniqueness guarantee. Reserve this fixture's
        // root exclusively; never borrow an existing directory that Drop can delete.
        fs::create_dir(&path).unwrap();
        let fixture = Self(path);
        // These programs use only print, so real Cargo builds need no downloads.
        fixture.write(
            "runtime/Cargo.toml",
            "[package]\nname='nagi-runtime'\nversion='0.1.0'\nedition='2021'\n[workspace]\n",
        );
        fixture.write("runtime/src/lib.rs", "");
        fixture
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn cli(&self, source: &str, output: &str, shared: bool) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_nagic"));
        command
            .current_dir(&self.0)
            .args(["run", source, "--no-project", "--out", output])
            .env("NAGI_ROOT", &self.0)
            .env("CARGO_NET_OFFLINE", "true")
            .env_remove("NAGI_NATIVE_TARGET_DIR");
        if shared {
            command.env("NAGI_NATIVE_TARGET_DIR", self.0.join("native-target"));
        }
        command
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Running(Option<Child>, PathBuf);

impl Running {
    fn output(mut self) -> Output {
        self.0.take().unwrap().wait_with_output().unwrap()
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        // Also release Cargo's wrapper if an assertion aborts this test.
        let _ = fs::write(self.1.join("release"), "");
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn successful(output: &Output, expected: &str) -> PathBuf {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    let path = stderr
        .lines()
        .find_map(|line| line.strip_prefix("native: "))
        .expect("native artifact path must be reported");
    let path = PathBuf::from(path);
    assert!(path.is_file(), "{}", path.display());
    // macOS may spell the same temporary directory as /var or /private/var.
    // Compare artifact identity rather than the diagnostic's path spelling.
    fs::canonicalize(path).unwrap()
}

#[test]
fn repeated_timestamps_keep_fixture_ownership_distinct() {
    let first = Fixture::new_at(42);
    let second = Fixture::new_at(42);
    first.write("first-owner.txt", "first");
    second.write("second-owner.txt", "second");
    let first_path = first.0.clone();
    let second_path = second.0.clone();
    let same_directory = first_path == second_path;
    drop(first);
    let second_survived_first_drop = second_path.join("second-owner.txt").is_file();
    // Observe both same-name allocation and cross-owner deletion before failing.
    assert!(
        !same_directory && second_survived_first_drop,
        "same timestamp borrowed the same directory: {same_directory}; second sentinel survived first Drop: {second_survived_first_drop}; first={} second={}",
        first_path.display(), second_path.display()
    );
    assert!(!first_path.exists());
    assert_eq!(
        fs::read_to_string(second_path.join("second-owner.txt")).unwrap(),
        "second"
    );
    drop(second);
    assert!(!second_path.exists());
}

#[test]
fn explicitly_shared_targets_keep_application_identity_stable_and_generations_distinct() {
    shared_targets_keep_application_binaries_distinct_and_names_stable(true);
}

#[test]
fn default_shared_targets_keep_application_identity_stable_and_generations_distinct() {
    shared_targets_keep_application_binaries_distinct_and_names_stable(false);
}

fn shared_targets_keep_application_binaries_distinct_and_names_stable(shared: bool) {
    let fixture = Fixture::new();
    for project in ["first", "second"] {
        fixture.write(
            &format!("{project}/same.nagi"),
            &format!("def main():\n    print(\"{project}\")\n"),
        );
    }
    fixture.write(
        "wrapper.rs",
        r#"use std::{env, fs, path::PathBuf, process::{Command, exit}, thread, time::{Duration, Instant}};
fn main() {
    let status = Command::new(env::var_os("NAGI_TEST_REAL_CARGO").unwrap())
        .args(env::args_os().skip(1)).status().unwrap();
    if status.success() && env::var_os("NAGI_TEST_HOLD").is_some() {
        let signals = PathBuf::from(env::var_os("NAGI_TEST_SIGNALS").unwrap());
        fs::write(signals.join("compiled"), "").unwrap();
        let until = Instant::now() + Duration::from_secs(60);
        while !signals.join("release").exists() {
            if Instant::now() > until { exit(99); }
            thread::sleep(Duration::from_millis(10));
        }
    }
    exit(status.code().unwrap_or(1));
}
"#,
    );
    let wrapper = fixture.0.join("wrapper");
    fs::create_dir_all(&wrapper).unwrap();
    let cargo_wrapper = wrapper.join(format!("cargo{}", std::env::consts::EXE_SUFFIX));
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .arg(fixture.0.join("wrapper.rs"))
        .arg("--edition=2021")
        .arg("-o")
        .arg(&cargo_wrapper)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let path = std::env::join_paths(std::iter::once(wrapper).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap();
    let signals = fixture.0.join("signals");
    fs::create_dir_all(&signals).unwrap();
    let mut first = Running(
        Some(
            fixture
                .cli("first/same.nagi", "first/generated", shared)
                .env("PATH", path)
                .env("NAGI_TEST_REAL_CARGO", env!("CARGO"))
                .env("NAGI_TEST_HOLD", "1")
                .env("NAGI_TEST_SIGNALS", &signals)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        ),
        signals.clone(),
    );
    // Pause after Cargo releases its cache lock but before Nagi launches the
    // binary. A second real build must not replace the first application's file.
    let until = Instant::now() + Duration::from_secs(60);
    while !signals.join("compiled").exists() {
        if first.0.as_mut().unwrap().try_wait().unwrap().is_some() {
            let output = first.output();
            panic!(
                "first build stopped before the barrier: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert!(
            Instant::now() <= until,
            "first Cargo build did not reach the barrier"
        );
        thread::sleep(Duration::from_millis(10));
    }
    // With no override both commands share the same cwd/native-target. Distinct
    // sources and generated directories must still select distinct binaries.
    let second = fixture
        .cli("second/same.nagi", "second/generated", shared)
        .output()
        .unwrap();
    fs::write(signals.join("release"), "").unwrap();
    let first = first.output();
    let first_binary = successful(&first, "first");
    let second_binary = successful(&second, "second");
    assert_ne!(first_binary, second_binary);
    // Q-001 / ADR 007: cache location is stable, successful exe paths are not.
    // Preserve different-app separation and the canonical 16-hex package identity.
    let first_package = package(&fixture.0.join("first/generated"));
    let second_package = package(&fixture.0.join("second/generated"));
    assert_ne!(first_package, second_package);
    for package in [&first_package, &second_package] {
        let suffix = package.strip_prefix("nagi-same-").unwrap();
        assert_eq!(suffix.len(), 16);
        assert!(suffix.bytes().all(|ch| ch.is_ascii_hexdigit()));
    }
    let cache = fixture.0.join("native-target/release");
    for binary in [&first_binary, &second_binary] {
        assert!(cache.join(binary.file_name().unwrap()).is_file());
        assert!(!binary.starts_with(fs::canonicalize(&cache).unwrap()));
    }
    let first_bytes = fs::read(&first_binary).unwrap();

    // Equivalent spelling of the same source/output keeps the cache identity.
    let equivalent = fixture
        .cli(
            "first/../first/same.nagi",
            "first/generated/../generated",
            shared,
        )
        .current_dir(fixture.0.join("first/.."))
        .output()
        .unwrap();
    let equivalent_binary = successful(&equivalent, "first");
    assert_ne!(equivalent_binary, first_binary);
    assert_eq!(package(&fixture.0.join("first/generated")), first_package);
    assert_eq!(fs::read(&first_binary).unwrap(), first_bytes);
    let old = Command::new(&first_binary).output().unwrap();
    assert!(old.status.success());
    assert_eq!(String::from_utf8_lossy(&old.stdout).trim(), "first");
    let alternate = fixture
        .cli("first/same.nagi", "first/other-generated", shared)
        .output()
        .unwrap();
    assert_ne!(successful(&alternate, "first"), first_binary);
    assert_ne!(
        package(&fixture.0.join("first/other-generated")),
        first_package
    );

    // Default caches also separate application identities. Selecting the same
    // cache with an explicit override must not change the application's name.
    let default = fixture
        .cli("first/same.nagi", "first/default-generated", false)
        .output()
        .unwrap();
    let default_binary = successful(&default, "first");
    let default_package = package(&fixture.0.join("first/default-generated"));
    let default_bytes = fs::read(&default_binary).unwrap();
    let explicit = fixture
        .cli("first/same.nagi", "first/default-generated", true)
        .output()
        .unwrap();
    let explicit_binary = successful(&explicit, "first");
    assert_ne!(explicit_binary, default_binary);
    assert_eq!(
        package(&fixture.0.join("first/default-generated")),
        default_package
    );
    assert_eq!(fs::read(&default_binary).unwrap(), default_bytes);
    assert_ne!(default_binary, first_binary);
    assert_ne!(default_package, first_package);
    assert!(cache.join(default_binary.file_name().unwrap()).is_file());
    assert!(cache.join(explicit_binary.file_name().unwrap()).is_file());
    let suffix = default_package.strip_prefix("nagi-same-").unwrap();
    assert_eq!(suffix.len(), 16);
    assert!(suffix.bytes().all(|ch| ch.is_ascii_hexdigit()));
}

fn package(out: &std::path::Path) -> String {
    let manifest: toml::Value =
        toml::from_str(&fs::read_to_string(out.join("Cargo.toml")).unwrap()).unwrap();
    manifest["package"]["name"].as_str().unwrap().to_string()
}
