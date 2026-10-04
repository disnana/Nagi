use std::{fs, path::PathBuf, process::Command};

static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "nagi cli {} {}",
            std::process::id(),
            ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("nagi.toml"), "[broken manifest").unwrap();
        Self(root)
    }
    fn run(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_nagic"))
            .args(args)
            .current_dir(&self.0)
            .env("PATH", &self.0)
            .env("NAGI_ROOT", self.0.join("missing-runtime"))
            .output()
            .unwrap()
    }
    fn unchanged(&self) {
        assert_eq!(fs::read_dir(&self.0).unwrap().count(), 1);
    }
    fn program() -> Self {
        let f = Self::new();
        fs::remove_file(f.0.join("nagi.toml")).unwrap();
        fs::write(f.0.join("main.nagi"), "def main():\n    print(42)\n").unwrap();
        let runtime = f.0.join("missing-runtime/runtime");
        fs::create_dir_all(&runtime).unwrap();
        fs::write(
            runtime.join("Cargo.toml"),
            "[package]\nname='nagi-runtime'\nversion='0.1.0'\nedition='2021'\n",
        )
        .unwrap();
        f
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn version_works_without_project_runtime_or_build_tools() {
    let fixture = Fixture::new();
    for arg in ["--version", "-V", "version"] {
        let result = fixture.run(&[arg]);
        assert!(result.status.success(), "{result:?}");
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            format!("nagic {}\n", env!("CARGO_PKG_VERSION"))
        );
        assert!(result.stderr.is_empty());
    }
    fixture.unchanged();
}

#[test]
fn help_works_without_source_or_valid_project() {
    let fixture = Fixture::new();
    for args in [
        vec!["--help"],
        vec!["-h"],
        vec!["help"],
        vec!["check", "--help"],
        vec!["lower", "-h"],
        vec!["build", "--help"],
        vec!["run", "-h"],
        vec!["symbols", "--help"],
    ] {
        let result = fixture.run(&args);
        assert!(result.status.success(), "{result:?}");
        assert!(result.stderr.is_empty());
        let text = String::from_utf8(result.stdout).unwrap();
        for word in [
            "Usage:",
            "check",
            "build",
            "run",
            "version",
            "--version",
            "--project",
        ] {
            assert!(text.contains(word), "{text}");
        }
    }
    fixture.unchanged();
}

#[test]
fn version_rejects_unexpected_arguments() {
    let fixture = Fixture::new();
    let result = fixture.run(&["version", "missing.nagi"]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8(result.stderr)
        .unwrap()
        .contains("version takes no additional arguments"));
    fixture.unchanged();
}

#[test]
fn check_needs_no_cargo_and_build_reports_the_missing_tool() {
    let f = Fixture::program();
    let check = f.run(&["check", "main.nagi"]);
    assert!(check.status.success(), "{check:?}");
    for command in ["build", "run"] {
        let output = f.run(&[command, "main.nagi"]);
        assert!(!output.status.success());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(
            error.contains("Cargoが見つかりません") && error.contains("cargo --version"),
            "{error}"
        );
        assert!(!error.contains("Rust backend rejected program"), "{error}");
        assert!(!String::from_utf8(output.stdout)
            .unwrap()
            .contains("native:"));
    }
}

#[test]
fn an_unusable_cargo_is_not_reported_as_a_missing_rust_installation() {
    let f = Fixture::program();
    let cargo = f.0.join(format!("cargo{}", std::env::consts::EXE_SUFFIX));
    fs::write(&cargo, "not an executable").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&cargo, fs::Permissions::from_mode(0o600)).unwrap();
    }
    // The isolated PATH selects the fixture on Unix and Windows.
    let output = f.run(&["build", "main.nagi"]);
    assert!(!output.status.success());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("Cargoを起動できません"), "{error}");
    assert!(
        !error.contains("Cargoが見つかりません") && !error.contains("Rust/Cargoを導入"),
        "{error}"
    );
}

#[test]
fn cargo_failure_preserves_the_cause_without_blaming_the_nagi_program() {
    let f = Fixture::program();
    let source = f.0.join("fake_cargo.rs");
    fs::write(
        &source,
        "fn main() { eprintln!(\"test dependency download failure\"); std::process::exit(37); }\n",
    )
    .unwrap();
    let cargo = f.0.join(format!("cargo{}", std::env::consts::EXE_SUFFIX));
    let compiled = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .arg(&source)
        .arg("-o")
        .arg(&cargo)
        .output()
        .unwrap();
    assert!(compiled.status.success(), "{compiled:?}");
    let output = f.run(&["run", "main.nagi"]);
    assert!(!output.status.success());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(
        error.contains("test dependency download failure") && error.contains("Build failed."),
        "{error}"
    );
    assert!(!error.contains("Rust backend rejected program"), "{error}");
    assert!(!String::from_utf8(output.stdout)
        .unwrap()
        .contains("native:"));
}

#[test]
fn check_and_lower_put_status_on_stderr_and_keep_cost_report_machine_readable() {
    let fixture = Fixture::program();
    for command in ["check", "lower"] {
        let output = fixture.run(&[command, "main.nagi"]);
        assert!(output.status.success(), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stderr).starts_with("checked "));

        let output = fixture.run(&[command, "main.nagi", "--cost-report"]);
        assert!(output.status.success(), "{output:?}");
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let saved: serde_json::Value = serde_json::from_slice(
            &fs::read(fixture.0.join("build/main/cost-report.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(report, saved);
        assert!(String::from_utf8_lossy(&output.stderr).starts_with("checked "));
    }
}
