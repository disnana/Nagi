#[path = "support/conformance.rs"]
mod support;

#[test]
fn explicit_move_corpus_reaches_native_execution() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/conformance");
    let cases: Vec<_> = support::corpus(&root)
        .into_iter()
        .filter(|case| case.name == "explicit_move" || case.name == "explicit_move_low")
        .collect();
    assert_eq!(cases.len(), 2, "High/handwritten Low move corpus must be registered");
    if let Err(failure) = support::run_cases(&cases) {
        let artifact = support::save_failure(&support::failure_case(&failure), &failure);
        panic!("{} stage={}\n{}\nartifact={}", failure.name, failure.stage, failure.diagnostic, artifact.display());
    }
}

#[test]
fn corpus_and_bounded_generated_contracts_reach_native_execution() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/conformance");
    let mut cases = support::corpus(&root);
    let seed = support::env_number("NAGI_CONFORMANCE_SEED", 305419896, u64::MAX);
    let count = support::env_number("NAGI_CONFORMANCE_CASES", 24, 2048);
    cases.extend(support::generated(seed, count as usize));
    if let Err(failure) = support::run_cases(&cases) {
        let case = support::failure_case(&failure);
        let artifact = support::save_failure(&case, &failure);
        panic!(
            "{} stage={} expected={} seed={}\n{}\nartifact={}",
            failure.name,
            failure.stage,
            failure.expected,
            failure.seed,
            failure.diagnostic,
            artifact.display()
        );
    }
}

#[test]
fn failure_shrinking_preserves_the_checker_failure_instead_of_a_parse_error() {
    let case = support::Case {
        name: "shrinker_self_check".into(),
        source: "def irrelevant() -> i64:\n    return 42\ndef negative(value: u64) -> u64:\n    return -value\n".into(),
        high: true, expected: "compile-pass".into(), diagnostic: String::new(), line: 0,
        oracle: String::new(), seed: 7, generator: None,
    };
    let failure = support::pipeline(&case).unwrap_err();
    assert_eq!(failure.stage, "high-check");
    let reduced = support::minimize(&case, &failure, 96);
    assert!(
        reduced.len() < case.source.len(),
        "shrinking made no progress"
    );
    let mut smaller = case;
    smaller.source = reduced;
    let repeated = support::pipeline(&smaller).unwrap_err();
    assert_eq!(repeated.stage, failure.stage);
    assert!(repeated.diagnostic.contains("u64は符号反転できません"));
}

#[test]
fn backend_shrinking_does_not_turn_an_oracle_error_into_a_compiler_reproducer() {
    let case = support::Case {
        name: "oracle_shrinker_self_check".into(),
        source: "def evaluate() -> i64:\n    return 42\n".into(),
        high: true,
        expected: "compile-pass".into(),
        diagnostic: String::new(),
        line: 0,
        oracle: "assert_eq!(missing_oracle_entrypoint(), 42i64);".into(),
        seed: 7,
        generator: None,
    };
    let failure = support::run_cases(std::slice::from_ref(&case)).unwrap_err();
    assert_eq!(failure.stage, "rustc");
    assert!(failure.diagnostic.contains("E0425"));
    assert_eq!(support::minimize(&case, &failure, 8), case.source);
}

#[test]
fn batch_root_relocation_preserves_literals_raw_strings_and_comments() {
    let rust = r####"fn inspect() {
    crate::target();
    let normal = "crate::target() \\\"quoted\\\"";
    let raw = r##"crate::raw()"##;
    let bytes = br#"crate::bytes()"#;
    // crate::line_comment()
    /* crate::outer() /* crate::inner() */ crate::outer_again() */
    othercrate::target();
    crate::last();
}"####;
    let relocated = support::unit(3, "saved", rust, "");
    assert!(relocated.contains("crate::case_3_saved::target();"));
    assert!(relocated.contains("crate::case_3_saved::last();"));
    for line in rust
        .lines()
        .filter(|line| !line.trim().starts_with("crate::"))
    {
        assert!(
            relocated.contains(line),
            "changed literal/comment/identifier: {line}"
        );
    }
}

#[test]
fn native_process_deadline_reaps_a_hanging_child() {
    if std::env::var_os("NAGI_TEST_HANG_CHILD").is_some() {
        std::thread::sleep(std::time::Duration::from_millis(500));
        return;
    }
    let root =
        std::env::temp_dir().join(format!("nagi-deadline-self-check-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "native_process_deadline_reaps_a_hanging_child"])
        .env("NAGI_TEST_HANG_CHILD", "1");
    let result = support::bounded_process(
        &mut command,
        "runtime",
        std::time::Duration::from_millis(30),
        &root,
    );
    std::fs::remove_dir_all(root).unwrap();
    let (stage, diagnostic) = result.unwrap_err();
    assert_eq!(stage, "runtime-timeout");
    assert!(diagnostic.contains("deadline="));
}
