use std::{fs, path::PathBuf, process::Command};

static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi output safety space ü {} {}",
            std::process::id(),
            ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        let f = Self(path);
        f.write("main.nagi", "def main():\n    print(42)\n");
        f.write(
            "runtime/Cargo.toml",
            "[package]\nname='nagi-runtime'\nversion='0.1.0'\nedition='2021'\n",
        );
        f
    }
    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn run(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_nagic"))
            .args(args)
            .current_dir(&self.0)
            .env("NAGI_ROOT", &self.0)
            .env("PATH", self.0.join("no-tools"))
            .output()
            .unwrap()
    }
    fn snapshot(&self, names: &[&str]) -> Vec<Vec<u8>> {
        names
            .iter()
            .map(|name| fs::read(self.0.join(name)).unwrap())
            .collect()
    }
    fn link(&self, from: &str, to: &str) {
        let to = self.0.join(to);
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::hard_link(self.0.join(from), to).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn rejected(output: std::process::Output) {
    assert!(!output.status.success(), "{output:?}");
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("cannot overwrite input file"), "{error}");
}

#[test]
fn native_low_is_not_replaced_by_check_or_lower_output() {
    for command in ["check", "lower"] {
        let f = Fixture::new();
        f.write("main.nagi", "def main():\n    print(value())\n");
        f.write("generated.low", "fn value() -> i64 { return 42; }\n");
        let inputs = ["main.nagi", "generated.low"];
        let before = f.snapshot(&inputs);
        for _ in 0..2 {
            rejected(f.run(&[
                command,
                "main.nagi",
                "--native",
                "generated.low",
                "--out",
                ".",
            ]));
            assert_eq!(f.snapshot(&inputs), before);
        }
    }
}

#[test]
fn build_preflights_rust_adapter_before_writing_any_generated_files() {
    let f = Fixture::new();
    f.write(
        "main.nagi",
        "@rust(\"native::value\")\nextern def value() -> i64\ndef main():\n    print(value())\n",
    );
    f.write("src/main.rs", "pub fn value() -> i64 { 42 }\n");
    f.write("generated.low", "previous generated content\n");
    f.write(
        "Cargo.toml",
        "[package]\nname='handwritten'\nversion='1.0.0'\n",
    );
    let files = ["main.nagi", "src/main.rs", "generated.low", "Cargo.toml"];
    let before = f.snapshot(&files);
    rejected(f.run(&["build", "main.nagi", "--rust", "src/main.rs", "--out", "."]));
    assert_eq!(f.snapshot(&files), before);
}

#[test]
fn every_generated_file_rejects_a_hard_link_to_an_imported_source() {
    for (target, command, cost) in [
        ("generated.low", "check", false),
        ("cost-report.json", "check", true),
        ("src/main.rs", "build", false),
        ("Cargo.toml", "build", false),
        ("Cargo.lock", "build", false),
    ] {
        let f = Fixture::new();
        f.write(
            "main.nagi",
            "import \"models.nagi\" as models\ndef main():\n    print(42)\n",
        );
        f.write("models.nagi", "class Item:\n    value: i64\n");
        f.link("models.nagi", target);
        let inputs = ["main.nagi", "models.nagi", target];
        let before = f.snapshot(&inputs);
        let mut args = vec![command, "main.nagi", "--out", "."];
        if cost {
            args.push("--cost-report");
        }
        rejected(f.run(&args));
        assert_eq!(f.snapshot(&inputs), before);
        if target != "generated.low" {
            assert!(!f.0.join("generated.low").exists());
        }
    }
}

#[test]
fn custom_project_manifest_is_protected_before_generated_outputs() {
    let f = Fixture::new();
    f.write("project settings.toml", "entry='main.nagi'\n");
    f.link("project settings.toml", "cost-report.json");
    let inputs = ["main.nagi", "project settings.toml", "cost-report.json"];
    let before = f.snapshot(&inputs);
    rejected(f.run(&[
        "check",
        "--project",
        "project settings.toml",
        "--cost-report",
        "--out",
        ".",
    ]));
    assert_eq!(f.snapshot(&inputs), before);
    assert!(!f.0.join("generated.low").exists());
}

#[test]
fn map_formats_protect_hard_links_to_sources_manifest_and_rust_adapter() {
    for input in ["main.nagi", "models.nagi", "project.toml", "bridge.rs"] {
        for format in ["mermaid", "d2", "json", "html", "svg", "png"] {
            let f = Fixture::new();
            f.write(
                "main.nagi",
                "import \"models.nagi\" as models\ndef main():\n    print(42)\n",
            );
            f.write("models.nagi", "class Item:\n    value: i64\n");
            f.write(
                "project.toml",
                "entry='main.nagi'\n[rust]\nfile='bridge.rs'\n",
            );
            f.write("bridge.rs", "pub fn value() -> i64 { 42 }\n");
            let output = format!("graph.{format}");
            f.link(input, &output);
            let inputs = ["main.nagi", "models.nagi", "project.toml", "bridge.rs"];
            let before = f.snapshot(&inputs);
            rejected(f.run(&[
                "map",
                "--project",
                "project.toml",
                "--format",
                format,
                "--output",
                &output,
            ]));
            assert_eq!(f.snapshot(&inputs), before);
            assert!(!f.0.join("build").exists());
        }
    }
}

#[cfg(unix)]
#[test]
fn symlinked_generated_paths_preserve_all_inputs() {
    let f = Fixture::new();
    f.write(
        "main.nagi",
        "@rust(\"native::value\")\nextern def value() -> i64\ndef main():\n    print(value())\n",
    );
    f.write("bridge.rs", "pub fn value() -> i64 { 42 }\n");
    fs::create_dir_all(f.0.join("output/src")).unwrap();
    std::os::unix::fs::symlink(f.0.join("bridge.rs"), f.0.join("output/src/main.rs")).unwrap();
    let inputs = ["main.nagi", "bridge.rs"];
    let before = f.snapshot(&inputs);
    rejected(f.run(&[
        "build",
        "main.nagi",
        "--rust",
        "bridge.rs",
        "--out",
        "output",
    ]));
    assert_eq!(f.snapshot(&inputs), before);
    assert!(!f.0.join("output/generated.low").exists());
}

#[test]
fn repeated_checks_and_build_preparation_keep_existing_generated_outputs_usable() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(value())\n");
    f.write("native/helper.low", "fn value() -> i64 { return 42; }\n");
    let inputs = ["main.nagi", "native/helper.low"];
    let before = f.snapshot(&inputs);
    for _ in 0..2 {
        let output = f.run(&[
            "check",
            "main.nagi",
            "--native",
            "native/helper.low",
            "--out",
            "output",
        ]);
        assert!(output.status.success(), "{output:?}");
        assert_eq!(f.snapshot(&inputs), before);
    }
    let mut generated = None;
    for _ in 0..2 {
        let output = f.run(&[
            "build",
            "main.nagi",
            "--native",
            "native/helper.low",
            "--out",
            "output",
        ]);
        assert!(!output.status.success());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("Cargoが見つかりません"), "{error}");
        assert_eq!(f.snapshot(&inputs), before);
        let current = f.snapshot(&[
            "output/generated.low",
            "output/src/main.rs",
            "output/Cargo.toml",
        ]);
        if let Some(previous) = &generated {
            assert_eq!(&current, previous);
        }
        generated = Some(current);
    }
}

#[test]
fn loaded_empty_module_filters_produce_empty_type_and_call_graphs() {
    let f = Fixture::new();
    f.write(
        "main.nagi",
        "import \"empty.nagi\" as empty\ndef main():\n    print(42)\n",
    );
    f.write("empty.nagi", "# no declarations\n");
    for view in ["types", "calls"] {
        let output = f.run(&[
            "map",
            view,
            "main.nagi",
            "--module",
            "empty",
            "--format",
            "json",
        ]);
        assert!(output.status.success(), "{output:?}");
        let graph: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        for key in ["nodes", "edges", "groups"] {
            assert!(graph[key].as_array().unwrap().is_empty(), "{graph}");
        }
        let output = f.run(&[
            "map",
            view,
            "main.nagi",
            "--module",
            "empty",
            "--focus",
            "Missing",
        ]);
        assert!(!output.status.success());
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("No graph node matches focus"));
    }
    let output = f.run(&[
        "map",
        "modules",
        "main.nagi",
        "--module",
        "empty",
        "--format",
        "json",
    ]);
    assert!(output.status.success(), "{output:?}");
    let graph: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(graph["nodes"].as_array().unwrap().len(), 1);
    let output = f.run(&["map", "types", "main.nagi", "--module", "unknown"]);
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("No loaded module matches"));
    assert!(!f.0.join("build").exists());
}

#[test]
fn include_text_assets_are_protected_in_generated_and_map_outputs() {
    for (command, target) in [("check", "generated.low"), ("map", "graph.json")] {
        let f = Fixture::new();
        f.write(
            "main.nagi",
            "def main():\n    print(include_text(\"message.txt\"))\n",
        );
        f.write("message.txt", "user-owned embedded text\n");
        f.link("message.txt", target);
        let inputs = ["main.nagi", "message.txt", target];
        let before = f.snapshot(&inputs);
        let args = if command == "map" {
            vec!["map", "main.nagi", "--format", "json", "--output", target]
        } else {
            vec!["check", "main.nagi", "--out", "."]
        };
        rejected(f.run(&args));
        assert_eq!(f.snapshot(&inputs), before);
    }
}

#[test]
fn aliased_generated_outputs_are_rejected_without_partial_writes() {
    let cases = if cfg!(unix) {
        vec![false, true]
    } else {
        vec![false]
    };
    for symlink in cases {
        let f = Fixture::new();
        f.write("output/generated.low", "previous generated Low\n");
        f.write("output/Cargo.toml", "previous generated Cargo manifest\n");
        fs::create_dir_all(f.0.join("output/src")).unwrap();
        if symlink {
            #[cfg(unix)]
            std::os::unix::fs::symlink(
                f.0.join("output/generated.low"),
                f.0.join("output/src/main.rs"),
            )
            .unwrap();
        } else {
            f.link("output/generated.low", "output/src/main.rs");
        }
        let files = [
            "main.nagi",
            "output/generated.low",
            "output/src/main.rs",
            "output/Cargo.toml",
        ];
        let before = f.snapshot(&files);
        let output = f.run(&["build", "main.nagi", "--out", "output"]);
        assert!(!output.status.success());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("output files must be distinct"), "{error}");
        assert_eq!(f.snapshot(&files), before);
    }
}
