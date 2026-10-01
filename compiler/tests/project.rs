use nagic::project;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

static FIXTURE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "nagi project {} {} {}",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn write(&self, name: &str, text: &str) {
        let p = self.0.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    fn resolve(&self, args: &[&str]) -> Result<project::Options, String> {
        project::resolve(
            &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            &self.0,
        )
    }
    fn run(&self, cwd: &Path, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(cwd)
            .args(args)
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn nearest_project_uses_manifest_relative_paths_from_any_subdirectory() {
    let f = Fixture::new();
    f.write("nagi.toml", "entry = 'src/main.nagi'\nnative = ['native/math.low']\n[rust]\nfile = 'bridge.rs'\n[rust.dependencies]\nserde_json = '1.0'\n");
    f.write("bridge.rs", "");
    f.write("src/main.nagi", "def main():\n    print(42)\n");
    let root = fs::canonicalize(&f.0).unwrap();
    let options = project::resolve(&["check".into()], &f.0.join("src")).unwrap();
    assert_eq!(options.source, root.join("src/main.nagi"));
    assert_eq!(options.native, [root.join("native/math.low")]);
    assert_eq!(options.rust_file, Some(root.join("bridge.rs")));
    assert_eq!(options.rust_dependencies["serde_json"], "1.0");
    assert_eq!(options.out, root.join("build/main"));
    assert_eq!(options.project_root, Some(root));
    f.write("src/nagi.toml", "entry = 'nested.low'\n");
    assert_eq!(
        f.resolve(&["check", "--project", "src"]).unwrap().source,
        fs::canonicalize(f.0.join("src"))
            .unwrap()
            .join("nested.low")
    );
}

#[test]
fn cli_overrides_scalars_and_dependency_versions_and_adds_native_files() {
    let f = Fixture::new();
    f.write("nagi.toml", "entry = 'entry.nagi'\nnative = ['base.low']\n[rust]\nfile = 'missing.rs'\n[rust.dependencies]\nserde_json = '1.0'\n");
    f.write("other.rs", "");
    let root = fs::canonicalize(&f.0).unwrap();
    let options = f
        .resolve(&[
            "build",
            "--project",
            "nagi.toml",
            "override.nagi",
            "--rust",
            "other.rs",
            "--rust-dep",
            "serde_json=1.0.151",
            "--native",
            "extra.low",
            "--out",
            "output",
            "--cost-report",
        ])
        .unwrap();
    assert_eq!(options.source, f.0.join("override.nagi"));
    assert_eq!(options.rust_file, Some(root.join("other.rs")));
    assert_eq!(options.rust_dependencies["serde_json"], "1.0.151");
    assert_eq!(
        options.native,
        [root.join("base.low"), f.0.join("extra.low")]
    );
    assert_eq!(options.out, f.0.join("output"));
    assert!(options.cost);
}

#[test]
fn explicit_source_keeps_legacy_behavior_even_next_to_a_broken_manifest() {
    let f = Fixture::new();
    f.write("nagi.toml", "broken = true");
    let options = f.resolve(&["run", "main.nagi"]).unwrap();
    assert!(options.project_root.is_none());
    assert_eq!(options.out, f.0.join("build/main"));
    assert!(f.resolve(&["check", "main.nagi", "--no-project"]).is_ok());
    assert!(f.resolve(&["check"]).is_err());
}

#[test]
fn manifest_typos_wrong_types_duplicates_and_empty_paths_are_rejected() {
    let f = Fixture::new();
    for text in [
        "entry = 'main.nagi'\nentr = 'other.nagi'",
        "entry = 42",
        "entry = ''",
        "entry = 'main.txt'",
        "entry = 'main.nagi'\nnative = ['']",
        "entry = 'main.nagi'\nrust = 'native.rs'",
        "entry = 'main.nagi'\n[rust]\nfiile = 'native.rs'",
        "entry = 'main.nagi'\n[rust]\nfile = ''",
        "entry = 'main.nagi'\n[rust.dependencies]\nserde_json = 1",
        "entry = 'main.nagi'\nentry = 'other.nagi'",
        "entry = 'main.nagi'\n[rust.dependencies]\nfoo-bar = '1'\nfoo_bar = '2'",
        "entry = 'main.nagi'\n[rust.dependencies]\nnagi_runtime = '1'",
        "entry = 'main.nagi'\n[rust.dependencies]\nfoo = ' '",
    ] {
        f.write("nagi.toml", text);
        let error = f.resolve(&["check"]).unwrap_err();
        assert!(error.contains("nagi.toml"), "{text}: {error}");
    }
}

#[test]
fn invalid_commands_and_flags_fail_before_loading_sources() {
    let f = Fixture::new();
    for args in [
        vec!["wat", "missing.nagi"],
        vec!["run", "--rust"],
        vec!["run", "--project", "--out", "x"],
        vec!["check", "a.nagi", "b.nagi"],
        vec!["check", "a.nagi", "--out", "x", "--out", "y"],
        vec!["check", "--project", ".", "--no-project"],
        vec!["check", "--no-project"],
        vec![
            "check",
            "main.nagi",
            "--rust-dep",
            "serde=1",
            "--rust-dep",
            "serde=2",
        ],
        vec![
            "check",
            "main.nagi",
            "--rust-dep",
            "foo-bar=1",
            "--rust-dep",
            "foo_bar=2",
        ],
        vec!["check", "main.nagi", "--rust-dep", "nagi-runtime=1"],
    ] {
        assert!(f.resolve(&args).is_err(), "{args:?}");
    }
}

#[test]
fn dependency_alias_collision_across_manifest_and_cli_is_rejected() {
    let f = Fixture::new();
    f.write(
        "nagi.toml",
        "entry = 'main.nagi'\n[rust.dependencies]\nfoo-bar = '1'\n",
    );
    assert!(f.resolve(&["check", "--rust-dep", "foo_bar=2"]).is_err());
}

#[test]
fn actual_cli_checks_entry_with_low_support_from_a_subdirectory() {
    let f = Fixture::new();
    f.write(
        "nagi.toml",
        "entry = 'src/main.nagi'\nnative = ['native/math.low']\n",
    );
    f.write("src/main.nagi", "def main():\n    print(twice(21))\n");
    f.write(
        "native/math.low",
        "fn twice(x: i64) -> i64 { return x * 2; }\n",
    );
    let result = f.run(&f.0.join("src"), &["check"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(f.0.join("build/main/generated.low").is_file());
    let standalone = f.run(&f.0, &["check", "src/main.nagi"]);
    assert!(
        !standalone.status.success(),
        "standalone must not silently use the project Low files"
    );
    let explicit = f.run(
        &f.0,
        &["lower", "--project", "nagi.toml", "--out", "custom"],
    );
    assert!(explicit.status.success());
    assert!(f.0.join("custom/generated.low").is_file());
}

#[test]
fn actual_cli_manifest_and_import_errors_point_to_original_files() {
    let f = Fixture::new();
    f.write(
        "nagi.toml",
        "entry = 'main.nagi'\n[rust]\nfiile = 'bridge.rs'\n",
    );
    let result = f.run(&f.0, &["check"]);
    assert!(
        !result.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stderr).contains("nagi.toml:3"));
    f.write("nagi.toml", "entry = 'main.nagi'\n");
    f.write(
        "main.nagi",
        "import \"helper.nagi\"\ndef main():\n    print(helper())\n",
    );
    f.write(
        "helper.nagi",
        "def helper() -> i32:\n    return \"wrong\"\n",
    );
    let result = f.run(&f.0, &["check"]);
    assert!(
        !result.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stderr).contains("helper.nagi:2"));
}

#[test]
fn actual_run_uses_rust_dependencies_and_a_stable_project_working_directory() {
    let f = Fixture::new();
    f.write("nagi.toml", "entry = 'src/project-smoke.nagi'\nnative = ['math.low']\n[rust]\nfile = 'native.rs'\n[rust.dependencies]\nserde_json = '1.0'\n");
    f.write("src/project-smoke.nagi", "@rust(\"native::record_cwd\")\nextern def record_cwd()\ndef main():\n    print(twice(21))\n    record_cwd()\n");
    f.write("math.low", "fn twice(x: i64) -> i64 { return x * 2; }\n");
    f.write("native.rs", "pub fn record_cwd() {\n    let cwd = std::env::current_dir().unwrap();\n    std::fs::write(\"cwd.json\", serde_json::to_string(&cwd).unwrap()).unwrap();\n}\n");
    let result = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .current_dir(f.0.join("src"))
        .arg("run")
        .env_remove("NAGI_NATIVE_TARGET_DIR")
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout)
        .lines()
        .any(|s| s == "42"));
    let cwd: PathBuf =
        serde_json::from_str(&fs::read_to_string(f.0.join("cwd.json")).unwrap()).unwrap();
    assert_eq!(
        fs::canonicalize(cwd).unwrap(),
        fs::canonicalize(&f.0).unwrap()
    );
    assert!(!f.0.join("src/cwd.json").exists());
    assert!(f
        .0
        .join(format!(
            "build/native-target/release/nagi-project-smoke{}",
            std::env::consts::EXE_SUFFIX
        ))
        .is_file());
    // A relative target override is resolved before run switches to the project cwd.
    let result = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .current_dir(f.0.join("src"))
        .arg("run")
        .env("CARGO_NET_OFFLINE", "true")
        .env("NAGI_NATIVE_TARGET_DIR", "../build/native-target")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn a_borrowed_record_field_does_not_block_moving_a_disjoint_field() {
    let f = Fixture::new();
    f.write("nagi.toml", "entry='main.nagi'\n");
    f.write("main.nagi", "class Data:\n    values: List[i64]\n    name: str\ndef main():\n    data = Data(values=[1, 2], name=\"Nagi\")\n    borrowed = view(data.values)\n    name = data.name\n    print(len(borrowed))\n    print(name)\n");
    let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("native-target")
        });
    let result = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .current_dir(&f.0)
        .arg("run")
        .env("NAGI_NATIVE_TARGET_DIR", target)
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert!(stdout.lines().any(|line| line == "2"));
    assert!(stdout.lines().any(|line| line == "Nagi"));
}
