use std::{
    fs,
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi shared cache 凪 {} {}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
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
fn explicitly_shared_targets_keep_application_binaries_distinct_and_names_stable() {
    shared_targets_keep_application_binaries_distinct_and_names_stable(true);
}

#[test]
fn default_shared_targets_keep_application_binaries_distinct_and_names_stable() {
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
    assert_eq!(first_binary.parent(), second_binary.parent());
    for binary in [&first_binary, &second_binary] {
        let name = binary.file_stem().unwrap().to_str().unwrap();
        let suffix = name.strip_prefix("nagi-same-").unwrap();
        assert_eq!(suffix.len(), 16);
        assert!(suffix.bytes().all(|ch| ch.is_ascii_hexdigit()));
    }

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
    assert_eq!(successful(&equivalent, "first"), first_binary);
    let alternate = fixture
        .cli("first/same.nagi", "first/other-generated", shared)
        .output()
        .unwrap();
    assert_ne!(successful(&alternate, "first"), first_binary);

    // Default caches also separate application identities. Selecting the same
    // cache with an explicit override must not change the application's name.
    let default = fixture
        .cli("first/same.nagi", "first/default-generated", false)
        .output()
        .unwrap();
    let default_binary = successful(&default, "first");
    let explicit = fixture
        .cli("first/same.nagi", "first/default-generated", true)
        .output()
        .unwrap();
    assert_eq!(successful(&explicit, "first"), default_binary);
    assert_ne!(default_binary, first_binary);
    assert_eq!(default_binary.parent(), first_binary.parent());
    let name = default_binary.file_stem().unwrap().to_str().unwrap();
    let suffix = name.strip_prefix("nagi-same-").unwrap();
    assert_eq!(suffix.len(), 16);
    assert!(suffix.bytes().all(|ch| ch.is_ascii_hexdigit()));
}
