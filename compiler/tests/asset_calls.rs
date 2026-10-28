#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, source};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "nagi asset calls {} {}",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn checked(&self, name: &str) -> source::Sources {
        let mut loaded = source::load(&self.0.join(name), name.ends_with(".nagi")).unwrap();
        check::check(&mut loaded.program).unwrap();
        loaded
    }

    fn run(&self, name: &str, program: &nagic::ast::Program, expected: &str) {
        let rust = self.0.join(format!("{name}.rs"));
        let binary = self
            .0
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        fs::write(&rust, emit::rust(&checked_emission::seal(program)).unwrap()).unwrap();
        let compiled = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2021", "-D", "warnings"])
            .arg(&rust)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let output = Command::new(binary).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
        assert!(output.stderr.is_empty());
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn local_callbacks_named_include_text_keep_literals_and_restore_builtin_after_scope() {
    let f = Fixture::new();
    fs::write(f.0.join("asset.txt"), "embedded").unwrap();
    fs::write(f.0.join("main.nagi"), "def identity(value: str) -> str:\n    return value\ndef callback(include_text: fn[str, str]) -> str:\n    return include_text(\"literal callback\")\ndef main():\n    print(include_text(\"asset.txt\"))\n    print(callback(identity))\n    if True:\n        include_text = identity\n        print(include_text(\"branch literal\"))\n    print(include_text(\"asset.txt\"))\n").unwrap();
    let high = f.checked("main.nagi");
    f.run(
        "high",
        &high.program,
        "embedded\nliteral callback\nbranch literal\nembedded\n",
    );
    fs::write(f.0.join("saved.low"), emit::low(&high.program)).unwrap();
    let low = f.checked("saved.low");
    f.run(
        "low",
        &low.program,
        "embedded\nliteral callback\nbranch literal\nembedded\n",
    );
}

#[test]
fn named_include_text_resolves_before_checker_normalization() {
    let f = Fixture::new();
    fs::write(f.0.join("asset.txt"), "embedded").unwrap();
    fs::write(
        f.0.join("main.nagi"),
        "def main():\n    print(include_text(path=\"asset.txt\"))\n",
    )
    .unwrap();
    let high = f.checked("main.nagi");
    let low = emit::low(&high.program);
    let canonical = fs::canonicalize(f.0.join("asset.txt"))
        .unwrap()
        .display()
        .to_string();
    let mut saved = nagic::parser::parse(&low, false).unwrap();
    check::check(&mut saved).unwrap();
    let nagic::ast::S::Expr(printed) = &saved.functions[0].body[0].kind else {
        panic!("{low}");
    };
    let nagic::ast::E::Call(_, _, arguments) = &printed.kind else {
        panic!("{low}");
    };
    let nagic::ast::E::Call(name, _, arguments) = &arguments[0].kind else {
        panic!("{low}");
    };
    assert_eq!(name, "include_text");
    assert!(
        matches!(&arguments[0].kind, nagic::ast::E::Str(path) if path == &canonical),
        "{low}"
    );
    assert!(!low.contains("include_text(\"asset.txt\")"), "{low}");
    f.run("named", &high.program, "embedded\n");
    f.run("named_saved", &saved, "embedded\n");
}

#[test]
fn named_local_include_text_is_not_resolved_as_an_asset() {
    let f = Fixture::new();
    fs::write(
        f.0.join("main.nagi"),
        "def identity(value: str) -> str:\n    return value\ndef callback(include_text: fn[str, str]) -> str:\n    return include_text(value=\"missing.txt\")\ndef main():\n    print(callback(identity))\n",
    )
    .unwrap();
    let mut loaded = source::load(&f.0.join("main.nagi"), true).unwrap();
    let error = check::check(&mut loaded.program).unwrap_err();
    assert!(
        error.contains("関数値の引数名は型に含まれません"),
        "{error}"
    );
}

#[test]
fn a_missing_real_asset_still_reports_the_original_call() {
    let f = Fixture::new();
    for (name, call) in [
        ("positional.nagi", "include_text(\"missing.txt\")"),
        ("named.nagi", "include_text(path=\"missing.txt\")"),
    ] {
        fs::write(f.0.join(name), format!("def main():\n    print({call})\n")).unwrap();
        let error = source::load(&f.0.join(name), true).err().unwrap();
        assert!(
            error.contains(&format!("{name}:2")) && error.contains("missing.txt"),
            "{error}"
        );
    }
}
