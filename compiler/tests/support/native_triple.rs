#![allow(dead_code)]
// 承認済みの狭い所有代入移行。型・move・origin拒否と実生成Rustを別に観測する。
#[path = "checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, source};
use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

fn locked_packages(text: &str) -> HashSet<(String, String, Option<String>)> {
    let lock: toml::Value = toml::from_str(text).unwrap();
    lock["package"]
        .as_array()
        .unwrap()
        .iter()
        .map(|package| {
            (
                package["name"].as_str().unwrap().to_owned(),
                package["version"].as_str().unwrap().to_owned(),
                package
                    .get("source")
                    .map(|source| source.as_str().unwrap().to_owned()),
            )
        })
        .collect()
}

pub(crate) struct Fixture(pub(crate) PathBuf);
impl Fixture {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "nagi-explicit-moves-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("exclusive move fixture: {error}"),
            }
        }
    }
    pub(crate) fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }
    pub(crate) fn checked(&self, name: &str) -> Result<nagic::ast::Program, String> {
        let mut loaded = source::load(&self.0.join(name), name.ends_with(".nagi"))?;
        check::check(&mut loaded.program)?;
        Ok(loaded.program)
    }
    pub(crate) fn reject(&self, high: &str, low: &str, line: usize, reason: &str) {
        for (name, text) in [("bad.nagi", high), ("bad.low", low)] {
            self.write(name, text);
            let error = self.checked(name).expect_err(text);
            assert!(
                error.starts_with(&format!("line {line}:")),
                "{name}: {error}\n{text}"
            );
            assert!(error.contains(reason), "{name}: {error}\n{text}");
        }
    }
    pub(crate) fn run_three(
        &self,
        case: &str,
        high: &str,
        low: &str,
        adapter: &str,
        assertions: &str,
    ) {
        self.write("main.nagi", high);
        let program = self
            .checked("main.nagi")
            .expect("High check before backend");
        self.write("saved.low", &emit::low(&program));
        self.write("handwritten.low", low);
        let saved = self
            .checked("saved.low")
            .expect("independent saved Low check");
        let handwritten = self
            .checked("handwritten.low")
            .expect("handwritten Low check");
        if let Some(directory) = std::env::var_os("NAGI_TEST_ARTIFACT_DIR") {
            let directory = PathBuf::from(directory);
            fs::create_dir_all(&directory).unwrap();
            for (name, source) in [("high.nagi", high), ("handwritten.low", low)] {
                fs::write(
                    directory.join(format!("explicit-move-{case}-{name}")),
                    source,
                )
                .unwrap();
            }
            fs::copy(
                self.0.join("saved.low"),
                directory.join(format!("explicit-move-{case}-saved.low")),
            )
            .unwrap();
        }
        // 保存Lowの再読込は元High fileへ依存してはならない。
        fs::remove_file(self.0.join("main.nagi")).unwrap();
        let independent = self
            .checked("saved.low")
            .expect("saved Low after High removal");
        assert_eq!(emit::low(&saved), emit::low(&independent));
        // workspaceと同じ依存版から始め、fixture packageだけCargoに追記させる。
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let lock = fs::read_to_string(repository.parent().unwrap().join("Cargo.lock")).unwrap();
        self.write("Cargo.lock", &lock);
        let approved = locked_packages(&lock);
        for (name, program) in [
            ("high", program),
            ("saved-low", independent),
            ("handwritten-low", handwritten),
        ] {
            let generated =
                emit::rust(&checked_emission::seal(&program)).expect("sealed Rust emission");
            if let Some(directory) = std::env::var_os("NAGI_TEST_ARTIFACT_DIR") {
                fs::write(
                    PathBuf::from(directory).join(format!("explicit-move-{case}-{name}-raw.rs")),
                    &generated,
                )
                .unwrap();
            }
            let rust = self.0.join(format!("{name}.rs"));
            fs::write(&rust, format!("{generated}\n{adapter}\n{assertions}")).unwrap();
            let package = self.0.file_name().unwrap().to_str().unwrap();
            let runtime = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("runtime");
            // 実runtimeのserde等へリンクする。生成Rustそのものは書き換えない。
            let runtime = toml::Value::String(runtime.to_str().unwrap().to_owned());
            self.write("Cargo.toml", &format!(
                "[package]\nname = \"{package}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n[lib]\npath = \"{name}.rs\"\n[dependencies]\nnagi-runtime = {{ path = {runtime} }}\ntokio = {{ version = \"1.48\", features = [\"rt-multi-thread\",\"sync\",\"time\"] }}\n[profile.dev]\ndebug = 0\n[profile.test]\ndebug = 0\n"
            ));
            let mut build =
                Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
            build
                .current_dir(&self.0)
                .args([
                    "test",
                    "--offline",
                    "--manifest-path",
                    "Cargo.toml",
                    "--lib",
                    "--no-run",
                    "--message-format=json",
                ])
                .env("CARGO_PROFILE_DEV_DEBUG", "0")
                .env("CARGO_PROFILE_TEST_DEBUG", "0")
                .env("CARGO_INCREMENTAL", "0");
            // 親が使用中のwarm cacheを使い、新しいfixture targetを作らない。
            let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
                .or_else(|| std::env::var_os("CARGO_TARGET_DIR"))
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .parent()
                        .unwrap()
                        .join("target")
                });
            let target = if target.is_absolute() {
                target
            } else {
                std::env::current_dir().unwrap().join(target)
            };
            build.env("CARGO_TARGET_DIR", target);
            let compiled = self.run_bounded(
                &mut build,
                &format!("{name}-build"),
                Duration::from_secs(180),
            );
            if let Some(directory) = std::env::var_os("NAGI_TEST_ARTIFACT_DIR") {
                let directory = PathBuf::from(directory);
                fs::write(
                    directory.join(format!("explicit-move-{case}-{name}-build.stdout")),
                    &compiled.stdout,
                )
                .unwrap();
                fs::write(
                    directory.join(format!("explicit-move-{case}-{name}-build.stderr")),
                    &compiled.stderr,
                )
                .unwrap();
            }
            assert!(
                compiled.status.success(),
                "{name} backend Cargo build:\n{}{}\n{generated}",
                String::from_utf8_lossy(&compiled.stdout),
                String::from_utf8_lossy(&compiled.stderr)
            );
            for dependency in
                locked_packages(&fs::read_to_string(self.0.join("Cargo.lock")).unwrap())
            {
                if dependency.0 == package {
                    assert_eq!(dependency.1, "0.0.0");
                    assert_eq!(dependency.2, None);
                } else {
                    assert!(
                        approved.contains(&dependency),
                        "fixture changed locked dependency: {dependency:?}"
                    );
                }
            }
            let binary = String::from_utf8_lossy(&compiled.stdout)
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .find_map(|event| {
                    (event["reason"] == "compiler-artifact"
                        && event["target"]["name"] == package.replace('-', "_")
                        && event["profile"]["test"] == true)
                        .then(|| event["executable"].as_str().map(PathBuf::from))
                        .flatten()
                })
                .expect("Cargo must report this fixture's actual test executable");
            let ran = self.run_bounded(
                Command::new(&binary).arg("--nocapture"),
                &format!("{name}-run"),
                Duration::from_secs(15),
            );
            assert!(
                ran.status.success(),
                "{name} backend run:\n{}{}",
                String::from_utf8_lossy(&ran.stdout),
                String::from_utf8_lossy(&ran.stderr)
            );
            if let Some(directory) = std::env::var_os("NAGI_TEST_ARTIFACT_DIR") {
                let directory = PathBuf::from(directory);
                fs::create_dir_all(&directory).unwrap();
                fs::copy(
                    &rust,
                    directory.join(format!("explicit-move-{case}-{name}.rs")),
                )
                .unwrap();
                fs::write(
                    directory.join(format!("explicit-move-{case}-{name}.stdout")),
                    &ran.stdout,
                )
                .unwrap();
                fs::write(
                    directory.join(format!("explicit-move-{case}-{name}-run.stderr")),
                    &ran.stderr,
                )
                .unwrap();
            }
        }
    }
    fn run_bounded(&self, command: &mut Command, label: &str, timeout: Duration) -> Output {
        // ファイル捕捉で大量のcompiler診断によるpipe満杯を避ける。
        let stdout = self.0.join(format!("{label}.stdout"));
        let stderr = self.0.join(format!("{label}.stderr"));
        command
            .stdout(Stdio::from(fs::File::create(&stdout).unwrap()))
            .stderr(Stdio::from(fs::File::create(&stderr).unwrap()));
        struct Reap(Child);
        impl Drop for Reap {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut child = Reap(
            command
                .spawn()
                .unwrap_or_else(|error| panic!("{label} spawn: {error}")),
        );
        let deadline = Instant::now() + timeout;
        let status = loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.0.kill();
                child.0.wait().unwrap();
                panic!(
                    "{label} timed out: stdout={} stderr={}",
                    fs::read_to_string(&stdout).unwrap(),
                    fs::read_to_string(&stderr).unwrap()
                );
            }
            thread::sleep(Duration::from_millis(5));
        };
        Output {
            status,
            stdout: fs::read(stdout).unwrap(),
            stderr: fs::read(stderr).unwrap(),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
