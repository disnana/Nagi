use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "nagi dependencies 凪 {} {} {}",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let fixture = Self(root);
        // Generated programs here use only std and a local Rust adapter. The
        // empty runtime makes real Cargo coverage independent of registry/cache.
        fixture.write(
            "runtime/Cargo.toml",
            "[package]\nname='nagi-runtime'\nversion='0.1.0'\nedition='2021'\n[workspace]\n",
        );
        fixture.write("runtime/src/lib.rs", "");
        fixture.write("main.nagi", "def main():\n    print(42)\n");
        fixture
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn manifest(&self, dependencies: &str) {
        self.write(
            "nagi.toml",
            &format!("entry='main.nagi'\n[rust.dependencies]\n{dependencies}\n"),
        );
    }

    fn cli(&self, cwd: &Path, args: &[&str], cargo: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_nagic"));
        command
            .current_dir(cwd)
            .args(args)
            .env("NAGI_ROOT", &self.0)
            .env("NAGI_NATIVE_TARGET_DIR", self.0.join("native-target"))
            .env("CARGO_HOME", self.0.join("empty-cargo-home"))
            .env("CARGO_NET_OFFLINE", "true");
        if !cargo {
            command.env("PATH", "");
        }
        command.output().unwrap()
    }

    fn generated_manifest(&self, cwd: &Path, args: &[&str], out: &Path) -> toml::Value {
        let output = self.cli(cwd, args, false);
        assert!(
            !output.status.success() && stderr(&output).contains("Cargo"),
            "manifest generation should reach the missing-Cargo diagnostic: {}",
            stderr(&output)
        );
        toml::from_str(&fs::read_to_string(out.join("Cargo.toml")).unwrap()).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn success(output: &Output) {
    assert!(output.status.success(), "{}", stderr(output));
}

fn quoted(value: &str) -> String {
    // JSON basic strings are also valid TOML basic strings for these fixtures.
    serde_json::to_string(value).unwrap()
}

#[test]
fn check_and_symbols_accept_dependency_tables_without_cargo_or_checked_out_crates() {
    let fixture = Fixture::new();
    fixture.manifest(
        "legacy='1.0'\nversioned={version='2.0', features=[], default-features=true}\nlocal={path='not checked out/support', features=['fast'], default-features=false, package='actual-support'}\ncombined={path='another missing crate', version='>=1.0, <2.0'}",
    );
    for command in ["check", "lower", "symbols"] {
        let output = fixture.cli(&fixture.0, &[command], false);
        success(&output);
        if command == "symbols" {
            let symbols: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(symbols.is_object());
        }
    }
    assert!(!fixture.0.join("build/main/Cargo.toml").exists());
    assert!(!fixture.0.join("empty-cargo-home").exists());
}

#[test]
fn generated_manifest_preserves_legacy_versions_and_every_supported_table_field() {
    let fixture = Fixture::new();
    fixture.manifest(
        "legacy='^1.2'\nrenamed={version='>=2, <3', features=['fast', 'serde/derive'], default-features=false, package='actual-package'}\nlocal={path='missing local crate', version='1.2.3', default-features=true}",
    );
    let out = fixture.0.join("build/main");
    let manifest = fixture.generated_manifest(&fixture.0, &["build"], &out);
    let dependencies = &manifest["dependencies"];
    assert_eq!(dependencies["legacy"].as_str(), Some("^1.2"));
    let renamed = &dependencies["renamed"];
    assert_eq!(renamed["version"].as_str(), Some(">=2, <3"));
    assert_eq!(renamed["package"].as_str(), Some("actual-package"));
    assert_eq!(renamed["default-features"].as_bool(), Some(false));
    assert_eq!(
        renamed["features"].as_array().unwrap(),
        &[
            toml::Value::String("fast".into()),
            toml::Value::String("serde/derive".into())
        ]
    );
    assert_eq!(dependencies["local"]["version"].as_str(), Some("1.2.3"));
    assert_eq!(
        dependencies["local"]["default-features"].as_bool(),
        Some(true)
    );
    assert!(dependencies["local"]["path"]
        .as_str()
        .unwrap()
        .contains("missing local crate"));
    assert!(manifest["workspace"].is_table());
    assert!(dependencies["nagi-runtime"]["path"].is_str());
}

#[test]
fn invalid_dependency_tables_fail_in_the_public_cli_before_cargo() {
    let fixture = Fixture::new();
    for dependencies in [
        "support={version='1', unknown=true}",
        "support={version='1', default_features=false}",
        "support=42",
        "support={version=42}",
        "support={path=42}",
        "support={version='1', features='fast'}",
        "support={version='1', features=[1]}",
        "support={version='1', default-features='false'}",
        "support={version='1', package=1}",
        "support={}",
        "support={features=['fast']}",
        "support=''",
        "support=' '",
        "support={version=''}",
        "support={path=''}",
        "support={version='1', path=' '}",
        "support={path='missing', version=' '}",
        "support={version='1', features=['']}",
        "support={version='1', features=[' ']}",
        "support={version='1', package=' '}",
        "support={version='1', package='bad package'}",
        "'bad name'={version='1'}",
        "'1bad'={version='1'}",
        "nagi-runtime={version='1'}",
        "nagi_runtime={path='missing'}",
        "renamed={version='1', package='nagi-runtime'}",
        "renamed={path='missing', package='nagi_runtime'}",
        "foo-bar={version='1'}\nfoo_bar='2'",
    ] {
        fixture.manifest(dependencies);
        let output = fixture.cli(&fixture.0, &["check"], false);
        let error = stderr(&output);
        assert!(!output.status.success(), "accepted {dependencies}");
        assert!(error.contains("nagi.toml"), "{dependencies}: {error}");
        assert!(
            !error.contains("Cargoが見つかりません"),
            "{dependencies}: {error}"
        );
    }
    assert!(!fixture.0.join("build/main/Cargo.toml").exists());
}

#[test]
fn cli_version_replaces_the_whole_table_and_keeps_alias_collision_checks() {
    let fixture = Fixture::new();
    fixture.manifest(
        "support={path='missing old path', version='1', features=['old-feature'], default-features=false, package='old-package'}",
    );
    success(&fixture.cli(&fixture.0, &["check", "--rust-dep", "support=2.3"], false));
    let manifest = fixture.generated_manifest(
        &fixture.0,
        &["build", "--rust-dep", "support=2.3"],
        &fixture.0.join("build/main"),
    );
    assert_eq!(manifest["dependencies"]["support"].as_str(), Some("2.3"));
    fixture.manifest("foo-bar={version='1'}");
    let output = fixture.cli(&fixture.0, &["check", "--rust-dep", "foo_bar=2"], false);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("重複"), "{}", stderr(&output));

    // CLI replacement does not make malformed manifest values valid.
    fixture.manifest("support={path='missing', features=['']}");
    let output = fixture.cli(&fixture.0, &["check", "--rust-dep", "support=2"], false);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("nagi.toml"), "{}", stderr(&output));
}

#[test]
fn missing_local_dependency_is_valid_for_check_and_reported_by_cargo_during_build() {
    let fixture = Fixture::new();
    fixture.manifest("support={path='not checked out'}");
    success(&fixture.cli(&fixture.0, &["check"], false));
    let output = fixture.cli(&fixture.0, &["build"], true);
    let error = stderr(&output);
    assert!(!output.status.success());
    assert!(fixture.0.join("build/main/Cargo.toml").is_file());
    assert!(
        error.contains("Cargo.toml") && error.contains("failed"),
        "{error}"
    );
    assert!(!error.contains("Cargoが見つかりません"), "{error}");
}

#[test]
fn dependency_paths_follow_the_selected_manifest_with_different_cwd_and_output() {
    let fixture = Fixture::new();
    fixture.write("project/src/main.nagi", "def main():\n    print(42)\n");
    fixture.write(
        "project/nagi.toml",
        "entry='src/main.nagi'\n[rust.dependencies]\nsupport={path='../local crates/凪 support'}\n",
    );
    fixture.write(
        "local crates/凪 support/Cargo.toml",
        "[package]\nname='support'\nversion='1.0.0'\nedition='2021'\n[workspace]\n",
    );
    fixture.write("local crates/凪 support/src/lib.rs", "");
    fs::create_dir_all(fixture.0.join("unrelated cwd")).unwrap();
    let expected = fs::canonicalize(fixture.0.join("local crates/凪 support")).unwrap();
    for (cwd, project, output) in [
        (
            fixture.0.join("project/src"),
            "../nagi.toml",
            "../../outside output/one",
        ),
        (
            fixture.0.join("unrelated cwd"),
            "../project/nagi.toml",
            "../outside output/two",
        ),
    ] {
        let out = cwd.join(output);
        let manifest = fixture.generated_manifest(
            &cwd,
            &["build", "--project", project, "--out", output],
            &out,
        );
        let emitted = manifest["dependencies"]["support"]["path"]
            .as_str()
            .unwrap();
        assert_eq!(fs::canonicalize(out.join(emitted)).unwrap(), expected);
    }
}

#[test]
fn generated_toml_round_trips_quotes_backslashes_and_newlines_without_extra_tables() {
    let fixture = Fixture::new();
    let version = "1\"\\\n[dependencies.injected]\nversion=\"999\"";
    let feature = "quote\" slash\\ newline\n tab\t 凪";
    fixture.manifest(&format!(
        "support={{version={}, features=[{}], default-features=false, package='real-support'}}",
        quoted(version),
        quoted(feature)
    ));
    let manifest =
        fixture.generated_manifest(&fixture.0, &["build"], &fixture.0.join("build/main"));
    let dependencies = manifest["dependencies"].as_table().unwrap();
    assert_eq!(dependencies.len(), 2);
    assert!(!dependencies.contains_key("injected"));
    assert_eq!(dependencies["support"]["version"].as_str(), Some(version));
    assert_eq!(
        dependencies["support"]["features"][0].as_str(),
        Some(feature)
    );
    assert_eq!(
        dependencies["support"]["package"].as_str(),
        Some("real-support")
    );
}

// macOS filesystems reject the non-UTF-8 name before the compiler can inspect it.
#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn non_utf8_project_paths_check_but_cannot_generate_lossy_dependency_paths() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let fixture = Fixture::new();
    let project = fixture.0.join(OsString::from_vec(b"project-\xff".to_vec()));
    fs::create_dir_all(project.join("support/src")).unwrap();
    fs::write(project.join("main.nagi"), "def main():\n    print(42)\n").unwrap();
    fs::write(
        project.join("nagi.toml"),
        "entry='main.nagi'\n[rust.dependencies]\nsupport={path='support'}\n",
    )
    .unwrap();
    fs::write(
        project.join("support/Cargo.toml"),
        "[package]\nname='support'\nversion='1.0.0'\nedition='2021'\n[workspace]\n",
    )
    .unwrap();
    fs::write(project.join("support/src/lib.rs"), "").unwrap();

    success(&fixture.cli(&project, &["check"], false));
    let output = fixture.cli(&project, &["build"], false);
    let error = stderr(&output);
    assert!(!output.status.success());
    assert!(
        error.contains("Cargo.tomlの生成") && error.contains("UTF-8"),
        "{error}"
    );
    assert!(!error.contains("Cargoが見つかりません"), "{error}");
    assert!(!project.join("build/main/Cargo.toml").exists());
    assert!(!fixture.0.join("native-target").exists());
    assert!(!fixture.0.join("empty-cargo-home").exists());
}

#[test]
fn local_package_rename_features_and_disabled_defaults_run_without_registry_dependencies() {
    let fixture = Fixture::new();
    fixture.write(
        "support crate/Cargo.toml",
        "[package]\nname='nagi-fixture-support'\nversion='1.2.3'\nedition='2021'\n[workspace]\n[features]\ndefault=['default-on']\ndefault-on=[]\nenabled=[]\n",
    );
    fixture.write(
        "support crate/src/lib.rs",
        "#[cfg(feature=\"default-on\")]\ncompile_error!(\"dependency default features must be disabled\");\n#[cfg(not(feature=\"enabled\"))]\ncompile_error!(\"the requested dependency feature is missing\");\npub fn answer() -> i64 { include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/answer.rs\")) }\n",
    );
    fixture.write("support crate/answer.rs", "42\n");
    fixture.write(
        "native.rs",
        "pub fn answer() -> i64 { renamed_support::answer() }\n",
    );
    fixture.write(
        "main.nagi",
        "@rust(\"native::answer\")\nextern def answer() -> i64\ndef main():\n    print(answer())\n",
    );
    fixture.write(
        "nagi.toml",
        "entry='main.nagi'\n[rust]\nfile='native.rs'\n[rust.dependencies]\nrenamed_support={path='support crate', version='1.2.3', features=['enabled'], default-features=false, package='nagi-fixture-support'}\n",
    );
    let output = fixture.cli(&fixture.0, &["run"], true);
    success(&output);
    assert!(String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| line == "42"));
    let lock_path = fixture.0.join("build/main/Cargo.lock");
    let lock = fs::read(&lock_path).unwrap();
    let parsed: toml::Value = toml::from_str(std::str::from_utf8(&lock).unwrap()).unwrap();
    let packages = parsed["package"].as_array().unwrap();
    assert!(packages
        .iter()
        .any(|package| package["name"].as_str() == Some("nagi-fixture-support")));
    assert!(packages
        .iter()
        .all(|package| package.get("source").is_none()));
    success(&fixture.cli(&fixture.0, &["run"], true));
    assert_eq!(fs::read(&lock_path).unwrap(), lock);
    // Even a failed Cargo launch regenerates the manifest. Nagi must retain
    // the caller's existing lockfile instead of deleting it first.
    let mut marked_lock = lock;
    marked_lock.extend_from_slice(b"\n# caller-owned lockfile marker\n");
    fs::write(&lock_path, &marked_lock).unwrap();
    fixture.generated_manifest(&fixture.0, &["build"], &fixture.0.join("build/main"));
    assert_eq!(fs::read(&lock_path).unwrap(), marked_lock);
}
