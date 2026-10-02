use std::{fs, path::PathBuf, process::Command};

static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "nagi install 凪 {} {}",
            std::process::id(),
            ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn distribution(&self) -> PathBuf {
        self.write(
            "distribution/runtime/Cargo.toml",
            "[package]\nname='nagi-runtime'\nversion='0.1.0'\nedition='2021'\n[workspace]\n",
        );
        self.write("distribution/runtime/src/lib.rs", "");
        self.write("distribution/release.json", "{}");
        let exe = self
            .0
            .join("distribution")
            .join(format!("nagic{}", std::env::consts::EXE_SUFFIX));
        fs::copy(env!("CARGO_BIN_EXE_nagic"), &exe).unwrap();
        exe
    }
    fn command(&self, exe: &PathBuf) -> Command {
        let mut cmd = Command::new(exe);
        cmd.current_dir(&self.0)
            .env_remove("NAGI_ROOT")
            .env("CARGO_NET_OFFLINE", "true")
            .env_remove("NAGI_NATIVE_TARGET_DIR");
        cmd
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn moved_distribution_builds_an_external_project_with_no_root_override() {
    let f = Fixture::new();
    let exe = f.distribution();
    f.write("separate project/nagi.toml", "entry='main.nagi'\n");
    f.write(
        "separate project/main.nagi",
        "def main():\n    print(\"Hello, outside!\")\n",
    );
    // A runtime in the caller's directory must not replace the bundled one.
    f.write("runtime/Cargo.toml", "not valid TOML\n");
    let result = f
        .command(&exe)
        .args(["run", "--project"])
        .arg(f.0.join("separate project/nagi.toml"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("Hello, outside!"));
    assert!(f
        .0
        .join("separate project/build/native-target/release")
        .is_dir());
    assert!(!f.0.join("distribution/native-target").exists());
}

#[test]
fn standalone_build_keeps_native_artifacts_in_the_working_directory() {
    let f = Fixture::new();
    let exe = f.distribution();
    f.write("main.nagi", "def main():\n    print(42)\n");
    let result = f.command(&exe).args(["run", "main.nagi"]).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(f.0.join("native-target/release").is_dir());
    assert!(!f.0.join("distribution/native-target").exists());
}

#[test]
fn wrong_root_explains_the_distribution_folder_before_calling_cargo() {
    let f = Fixture::new();
    let exe = f.distribution();
    f.write("main.nagi", "def main():\n    print(1)\n");
    let result = f
        .command(&exe)
        .args(["build", "main.nagi"])
        .env("NAGI_ROOT", f.0.join("distribution/target/release"))
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(!result.status.success());
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(
        error.contains("NAGI_ROOTにruntime/Cargo.tomlがありません"),
        "{error}"
    );
    assert!(error.contains("展開フォルダー"), "{error}");
    assert!(!error.contains("cargo/rustc"), "{error}");
    assert!(!f.0.join("build/main/Cargo.toml").exists());
}
