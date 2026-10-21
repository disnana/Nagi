use serde_json::Value;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    cwd: PathBuf,
    output: PathBuf,
    cargo: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        for _ in 0..100 {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "nagi-cache-inspect-{}-{now}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&root) {
                Ok(()) => {
                    let cwd = root.join("cwd");
                    let output = root.join("output cache");
                    let cargo = root.join("cargo cache");
                    fs::create_dir(&cwd).unwrap();
                    fs::create_dir(&output).unwrap();
                    fs::create_dir(&cargo).unwrap();
                    fs::write(cwd.join("nagi.toml"), "[broken manifest").unwrap();
                    fs::write(cwd.join("main.nagi"), "this must not be parsed").unwrap();
                    return Self {
                        root,
                        cwd,
                        output,
                        cargo,
                    };
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("cannot reserve an exclusive test directory: {error}"),
            }
        }
        panic!("cannot reserve an exclusive test directory");
    }

    fn run(&self, args: &[OsString]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_nagic"))
            .args(args)
            .current_dir(&self.cwd)
            .env("PATH", self.root.join("empty-path"))
            .env("NAGI_ROOT", self.root.join("missing-runtime"))
            .env("NAGI_NATIVE_TARGET_DIR", self.root.join("env-cargo-cache"))
            .env("CARGO_HOME", self.root.join("env-cargo-home"))
            .env("CARGO_TARGET_DIR", self.root.join("env-cargo-target"))
            .output()
            .unwrap()
    }

    fn inspect_args(&self) -> Vec<OsString> {
        vec![
            "cache".into(),
            "inspect".into(),
            "--out-root".into(),
            self.output.as_os_str().to_owned(),
            "--cargo-root".into(),
            self.cargo.as_os_str().to_owned(),
            "--max-entries".into(),
            "128".into(),
            "--max-depth".into(),
            "8".into(),
        ]
    }

    fn snapshot(&self, path: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        fn walk(root: &Path, path: &Path, result: &mut Vec<(PathBuf, Vec<u8>)>) {
            let metadata = fs::symlink_metadata(path).unwrap();
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            if metadata.is_dir() {
                result.push((relative, Vec::new()));
                let mut children: Vec<_> = fs::read_dir(path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path())
                    .collect();
                children.sort();
                for child in children {
                    walk(root, &child, result);
                }
            } else if metadata.file_type().is_symlink() {
                result.push((
                    relative,
                    fs::read_link(path)
                        .unwrap()
                        .as_os_str()
                        .as_encoded_bytes()
                        .to_vec(),
                ));
            } else {
                result.push((relative, fs::read(path).unwrap()));
            }
        }

        let mut result = Vec::new();
        walk(path, path, &mut result);
        result
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap()
}

fn assert_syntax_error(output: Output) {
    assert!(!output.status.success(), "{output:?}");
    assert!(
        output.stdout.is_empty(),
        "syntax errors must precede scanning"
    );
    assert!(!output.stderr.is_empty());
}

#[test]
fn global_help_and_version_still_precede_cache_dispatch() {
    let fixture = Fixture::new();
    let before_cwd = fixture.snapshot(&fixture.cwd);
    let before_output = fixture.snapshot(&fixture.output);
    let before_cargo = fixture.snapshot(&fixture.cargo);

    let help = fixture.run(&["cache".into(), "--help".into()]);
    assert!(help.status.success(), "{help:?}");
    assert!(String::from_utf8_lossy(&help.stdout).contains("nagic cache inspect"));
    let version = fixture.run(&["version".into()]);
    assert!(version.status.success(), "{version:?}");
    assert_eq!(
        String::from_utf8_lossy(&version.stdout),
        format!("nagic {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(fixture.snapshot(&fixture.cwd), before_cwd);
    assert_eq!(fixture.snapshot(&fixture.output), before_output);
    assert_eq!(fixture.snapshot(&fixture.cargo), before_cargo);
}

#[test]
fn inspect_requires_all_options_once_and_rejects_non_inspect_arguments_before_io() {
    let fixture = Fixture::new();
    let before_cwd = fixture.snapshot(&fixture.cwd);
    let before_output = fixture.snapshot(&fixture.output);
    let before_cargo = fixture.snapshot(&fixture.cargo);
    let valid = fixture.inspect_args();

    for option_index in [2, 4, 6, 8] {
        let mut args = valid.clone();
        args.drain(option_index..option_index + 2);
        assert_syntax_error(fixture.run(&args));
    }

    let mut cases = Vec::new();
    let mut duplicate_root = valid.clone();
    duplicate_root.extend(["--out-root".into(), fixture.output.as_os_str().to_owned()]);
    cases.push(duplicate_root);
    let mut duplicate_limit = valid.clone();
    duplicate_limit.extend(["--max-depth".into(), "2".into()]);
    cases.push(duplicate_limit);
    let mut relative_root = valid.clone();
    relative_root[3] = "relative-output".into();
    cases.push(relative_root);
    for flag in ["--project", "--editor-input", "--out", "--native", "--rust"] {
        let mut unknown = valid.clone();
        unknown.extend([flag.into(), "value".into()]);
        cases.push(unknown);
    }
    for (flag, value) in [
        ("--max-entries", "0"),
        ("--max-entries", "4097"),
        ("--max-depth", "0"),
        ("--max-depth", "65"),
        ("--max-depth", "not-a-number"),
    ] {
        let mut invalid_limit = valid.clone();
        let index = invalid_limit.iter().position(|arg| arg == flag).unwrap();
        invalid_limit[index + 1] = value.into();
        cases.push(invalid_limit);
    }
    let mut source = valid.clone();
    source.push("main.nagi".into());
    cases.push(source);
    let mut extra_subcommand = valid.clone();
    extra_subcommand.push("prune".into());
    cases.push(extra_subcommand);
    cases.push(vec!["cache".into(), "prune".into()]);

    for args in cases {
        assert_syntax_error(fixture.run(&args));
    }
    assert_eq!(fixture.snapshot(&fixture.output), before_output);
    assert_eq!(fixture.snapshot(&fixture.cargo), before_cargo);
    assert_eq!(fixture.snapshot(&fixture.cwd), before_cwd);
    assert!(!fixture.root.join("env-cargo-cache").exists());
    assert!(!fixture.root.join("env-cargo-home").exists());
    assert!(!fixture.root.join("env-cargo-target").exists());
    assert!(!fixture.output.join(".nagi-write.lock").exists());
}

#[test]
fn oversized_unicode_root_is_rejected_before_json_or_filesystem_access() {
    let fixture = Fixture::new();
    let before_cwd = fixture.snapshot(&fixture.cwd);
    let before_output = fixture.snapshot(&fixture.output);
    let before_cargo = fixture.snapshot(&fixture.cargo);
    let oversized = std::env::temp_dir().join("é".repeat(2050));
    assert!(oversized.as_os_str().as_encoded_bytes().len() > 4096);
    assert!(oversized.to_string_lossy().chars().count() < 4097);

    let mut args = fixture.inspect_args();
    args[3] = oversized.as_os_str().to_owned();
    let output = fixture.run(&args);
    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "syntax errors must precede scanning"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("4096 encoded path bytes"));
    assert_eq!(fixture.snapshot(&fixture.cwd), before_cwd);
    assert_eq!(fixture.snapshot(&fixture.output), before_output);
    assert_eq!(fixture.snapshot(&fixture.cargo), before_cargo);
    assert!(!fixture.root.join("env-cargo-cache").exists());
    assert!(!fixture.root.join("env-cargo-home").exists());
    assert!(!fixture.root.join("env-cargo-target").exists());
}

#[test]
fn inspect_reads_only_explicit_roots_and_emits_versioned_lossless_json() {
    let fixture = Fixture::new();
    fs::write(fixture.output.join("generated.low"), b"output marker\0\xff").unwrap();
    fs::write(fixture.cargo.join("artifact.rlib"), b"cargo marker\0\xfe").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        let name = std::ffi::OsString::from_vec(vec![b'r', 0xff, b'w']);
        let path = fixture.output.join(name);
        fs::write(&path, b"raw name").unwrap();
        let before_cwd = fixture.snapshot(&fixture.cwd);
        let before_output = fixture.snapshot(&fixture.output);
        let before_cargo = fixture.snapshot(&fixture.cargo);
        let output = fixture.run(&fixture.inspect_args());
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty());
        let report = json(&output);
        assert_eq!(report["format_version"], 1);
        assert_eq!(report["roots"]["output"]["kind"], "output");
        assert_eq!(report["roots"]["shared_cargo"]["kind"], "shared_cargo");
        assert_eq!(report["roots"]["output"]["enumeration_complete"], true);
        assert_eq!(report["roots"]["output"]["logical_bytes_complete"], true);
        assert_eq!(
            report["roots"]["output"]["root_os"]["units"],
            serde_json::to_value(fixture.output.as_os_str().as_bytes()).unwrap()
        );
        let raw_sample = report["roots"]["output"]["samples"]
            .as_array()
            .unwrap()
            .iter()
            .find(|sample| {
                sample["relative_path_os"]["units"] == serde_json::json!([114, 255, 119])
            })
            .expect("the raw filename bytes should be preserved in the sample");
        assert_eq!(raw_sample["relative_path_os"]["encoding"], "unix_bytes");
        assert_eq!(
            raw_sample["relative_path_os"]["units"],
            serde_json::to_value(path.file_name().unwrap().as_bytes()).unwrap()
        );
        let limitations = report["limitations"].as_array().unwrap();
        assert!(limitations
            .iter()
            .any(|item| item.as_str().unwrap().contains("physical allocation")));
        assert!(limitations
            .iter()
            .any(|item| item.as_str().unwrap().contains("atomic snapshot")));
        assert_eq!(fixture.snapshot(&fixture.output), before_output);
        assert_eq!(fixture.snapshot(&fixture.cargo), before_cargo);
        assert_eq!(fixture.snapshot(&fixture.cwd), before_cwd);
        assert!(!fixture.output.join(".nagi-write.lock").exists());
        assert!(!fixture.cwd.join("build").exists());
        assert!(!fixture.root.join("env-cargo-cache").exists());
        assert!(!fixture.root.join("env-cargo-home").exists());
        assert!(!fixture.root.join("env-cargo-target").exists());
    }
    #[cfg(not(unix))]
    {
        let before_cwd = fixture.snapshot(&fixture.cwd);
        let before_output = fixture.snapshot(&fixture.output);
        let before_cargo = fixture.snapshot(&fixture.cargo);
        let output = fixture.run(&fixture.inspect_args());
        assert!(output.status.success(), "{output:?}");
        let report = json(&output);
        assert_eq!(report["format_version"], 1);
        assert_eq!(report["roots"]["output"]["kind"], "output");
        assert_eq!(fixture.snapshot(&fixture.output), before_output);
        assert_eq!(fixture.snapshot(&fixture.cargo), before_cargo);
        assert_eq!(fixture.snapshot(&fixture.cwd), before_cwd);
    }
}

#[test]
fn missing_and_bounded_roots_emit_incomplete_json_and_fail() {
    let fixture = Fixture::new();
    let missing = fixture.root.join("missing output");
    let mut args = fixture.inspect_args();
    args[3] = missing.as_os_str().to_owned();
    let output = fixture.run(&args);
    assert!(!output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("cache inspect incomplete"));
    let report = json(&output);
    assert_eq!(report["roots"]["output"]["reasons"]["missing_root"], true);
    assert_eq!(report["roots"]["output"]["enumeration_complete"], false);
    assert!(!missing.exists());

    for index in 0..4 {
        fs::write(fixture.output.join(format!("entry-{index}")), [index as u8]).unwrap();
    }
    let mut bounded = fixture.inspect_args();
    let index = bounded
        .iter()
        .position(|arg| arg == "--max-entries")
        .unwrap();
    bounded[index + 1] = "1".into();
    let output = fixture.run(&bounded);
    assert!(!output.status.success(), "{output:?}");
    let report = json(&output);
    assert_eq!(
        report["roots"]["output"]["reasons"]["entry_limit_hit"],
        true
    );
    assert_eq!(report["roots"]["output"]["enumeration_complete"], false);
    assert!(
        report["roots"]["output"]["observed_entries"]
            .as_u64()
            .unwrap()
            <= 1
    );
}

#[test]
fn overlapping_and_aliased_roots_are_reported_without_combining_bytes() {
    let fixture = Fixture::new();
    let nested = fixture.output.join("nested cargo");
    fs::create_dir(&nested).unwrap();
    fs::write(nested.join("artifact"), b"cargo").unwrap();
    let mut args = fixture.inspect_args();
    args[5] = nested.as_os_str().to_owned();
    let output = fixture.run(&args);
    assert!(output.status.success(), "{output:?}");
    let report = json(&output);
    assert_eq!(report["roots_overlap_lexically"], true);
    assert!(
        report["roots"]["output"]["classes"]["unknown"]["logical_regular_file_bytes"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(report.get("total_bytes").is_none());
    assert!(report.get("unique_bytes").is_none());

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let alias = fixture.root.join("output-alias");
        symlink(&fixture.output, &alias).unwrap();
        let mut args = fixture.inspect_args();
        args[5] = alias.as_os_str().to_owned();
        let output = fixture.run(&args);
        assert!(!output.status.success());
        let report = json(&output);
        assert_eq!(report["roots_overlap_lexically"], false);
        assert_eq!(
            report["roots"]["shared_cargo"]["reasons"]["root_link_or_reparse"],
            true
        );
        assert!(report["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("Roots can alias on disk")));
    }
}

#[test]
fn sample_count_is_bounded_and_observed_links_make_logical_bytes_incomplete() {
    let fixture = Fixture::new();
    for index in 0..24 {
        fs::write(
            fixture.output.join(format!("sample-{index}")),
            [index as u8],
        )
        .unwrap();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(
            fixture.output.join("sample-0"),
            fixture.output.join("linked-file"),
        )
        .unwrap();
    }
    let before = fixture.snapshot(&fixture.output);
    let mut args = fixture.inspect_args();
    let index = args.iter().position(|arg| arg == "--max-entries").unwrap();
    args[index + 1] = "128".into();
    let output = fixture.run(&args);
    let report = json(&output);
    let root = &report["roots"]["output"];
    assert_eq!(root["enumeration_complete"], true);
    assert!(root["samples"].as_array().unwrap().len() <= 16);
    assert!(root["reasons"]["sample_omitted"].as_u64().unwrap() > 0);
    #[cfg(unix)]
    {
        assert!(
            !output.status.success(),
            "observed links make bytes incomplete"
        );
        assert_eq!(root["logical_bytes_complete"], false);
        assert_eq!(root["reasons"]["symlinks"], 1);
    }
    #[cfg(not(unix))]
    {
        assert!(output.status.success(), "ordinary files are complete");
        assert_eq!(root["logical_bytes_complete"], true);
    }
    assert_eq!(fixture.snapshot(&fixture.output), before);
}

#[test]
fn ordinary_file_sample_overflow_is_bounded_and_complete() {
    let fixture = Fixture::new();
    for index in 0..24 {
        fs::write(
            fixture.output.join(format!("sample-{index}")),
            [index as u8],
        )
        .unwrap();
    }

    let mut args = fixture.inspect_args();
    let index = args.iter().position(|arg| arg == "--max-entries").unwrap();
    args[index + 1] = "128".into();
    let output = fixture.run(&args);
    assert!(output.status.success(), "{output:?}");
    let root = json(&output)["roots"]["output"].clone();
    assert_eq!(root["enumeration_complete"], true);
    assert_eq!(root["logical_bytes_complete"], true);
    assert_eq!(root["samples"].as_array().unwrap().len(), 16);
    assert_eq!(root["reasons"]["sample_omitted"], 8);
}
