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
            .env("PATH", "")
            .env("NAGI_ROOT", self.0.join("missing-runtime"))
            .output()
            .unwrap()
    }
    fn unchanged(&self) {
        assert_eq!(fs::read_dir(&self.0).unwrap().count(), 1);
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
