//! 共通の段階別runner。accepted生成と任意text mutationのoracleは分離する。
use nagic::{check, emit, parser};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GeneratedInput {
    pub variant: usize,
    pub a: i64,
    pub b: i64,
    pub c: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Case {
    pub name: String,
    pub source: String,
    pub high: bool,
    pub expected: String,
    #[serde(default)]
    pub diagnostic: String,
    #[serde(default)]
    pub line: usize,
    #[serde(default)]
    pub oracle: String,
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub generator: Option<GeneratedInput>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Failure {
    pub name: String,
    pub source: String,
    pub seed: u64,
    pub stage: String,
    pub expected: String,
    pub diagnostic: String,
    pub high: bool,
    pub oracle: String,
    pub expected_diagnostic: String,
    pub expected_line: usize,
    pub generator: Option<GeneratedInput>,
}
impl Failure {
    fn new(case: &Case, stage: &str, diagnostic: impl Into<String>) -> Box<Self> {
        Box::new(Self {
            name: case.name.clone(),
            source: case.source.clone(),
            seed: case.seed,
            stage: stage.into(),
            expected: case.expected.clone(),
            diagnostic: diagnostic.into(),
            high: case.high,
            oracle: case.oracle.clone(),
            expected_diagnostic: case.diagnostic.clone(),
            expected_line: case.line,
            generator: case.generator.clone(),
        })
    }
}
fn step<T>(
    case: &Case,
    stage: &str,
    run: impl FnOnce() -> Result<T, String>,
) -> Result<T, Box<Failure>> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(run))
        .map_err(|panic| {
            let text = panic
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| panic.downcast_ref::<&str>().copied())
                .unwrap_or("unknown panic");
            Failure::new(case, stage, format!("panic: {text}"))
        })?
        .map_err(|error| Failure::new(case, stage, error))
}

/// 初期parse/checkの失敗はnegative oracleの対象。後段の拒否は常にfailure。
pub fn pipeline(case: &Case) -> Result<(String, String), Box<Failure>> {
    let prefix = if case.high { "high" } else { "low" };
    let mut program = step(case, &format!("{prefix}-parse"), || {
        parser::parse(&case.source, case.high)
    })?;
    program = resolve_imports(case, program)?;
    step(case, &format!("{prefix}-check"), || {
        check::check(&mut program)
    })?;
    let low_text = step(case, "low-pretty", || Ok(emit::low(&program)))?;
    let mut low = step(case, "low-reparse", || parser::parse(&low_text, false))?;
    step(case, "low-recheck", || check::check(&mut low))?;
    let direct_checked = step(case, "direct-finalize", || {
        check::finalize(
            program,
            nagic::ast::Program::default(),
            nagic::source::SourceProvenance::user_low_unmapped(),
        )
        .map_err(|error| format!("[{:?}] {error}", error.kind()))
    })?;
    let saved_checked = step(case, "saved-low-finalize", || {
        check::finalize(
            low,
            nagic::ast::Program::default(),
            nagic::source::SourceProvenance::user_low_unmapped(),
        )
        .map_err(|error| format!("[{:?}] {error}", error.kind()))
    })?;
    let direct = step(case, "rust-emit-direct", || emit::rust(&direct_checked))?;
    let saved = step(case, "rust-emit-saved-low", || emit::rust(&saved_checked))?;
    Ok((direct, saved))
}

/// 任意textは初期parse/checkの通常拒否を許容する。check成功後は後段を全て要求する。
pub enum MutationOutcome {
    ParseRejected,
    CheckRejected,
    Checked,
}
pub fn mutation(case: &Case) -> Result<MutationOutcome, Box<Failure>> {
    let prefix = if case.high { "high" } else { "low" };
    let mut program = match step(case, &format!("{prefix}-parse"), || {
        parser::parse(&case.source, case.high)
    }) {
        Ok(program) => program,
        Err(error) if !error.diagnostic.starts_with("panic:") => {
            return Ok(MutationOutcome::ParseRejected)
        }
        Err(error) => return Err(error),
    };
    program = match resolve_imports(case, program) {
        Ok(program) => program,
        Err(error) if !error.diagnostic.starts_with("panic:") => {
            return Ok(MutationOutcome::CheckRejected)
        }
        Err(error) => return Err(error),
    };
    match step(case, &format!("{prefix}-check"), || {
        check::check(&mut program)
    }) {
        Ok(()) => (),
        Err(error) if !error.diagnostic.starts_with("panic:") => {
            return Ok(MutationOutcome::CheckRejected)
        }
        Err(error) => return Err(error),
    }
    pipeline(case).map(|_| MutationOutcome::Checked)
}

pub fn negative(case: &Case) -> Result<(), Box<Failure>> {
    let stage = case
        .expected
        .strip_prefix("reject:")
        .expect("negative stage");
    let result = pipeline(case);
    match result {
        Err(error)
            if error.stage == stage
                && !error.diagnostic.starts_with("panic:")
                && error.diagnostic.contains(&case.diagnostic)
                && error
                    .diagnostic
                    .starts_with(&format!("line {}:", case.line)) =>
        {
            Ok(())
        }
        Err(error) => Err(Failure::new(
            case,
            "negative-oracle",
            format!(
                "expected {stage} line {} containing {:?}; actual {}: {}",
                case.line, case.diagnostic, error.stage, error.diagnostic
            ),
        )),
        Ok(_) => Err(Failure::new(
            case,
            "negative-oracle",
            "unexpected acceptance",
        )),
    }
}

pub fn corpus(root: &std::path::Path) -> Vec<Case> {
    let mut cases: Vec<Case> =
        serde_json::from_str(&fs::read_to_string(root.join("corpus.json")).unwrap()).unwrap();
    for case in &mut cases {
        case.source = fs::read_to_string(root.join(&case.source)).unwrap();
    }
    cases
}

pub fn env_number(name: &str, default: u64, maximum: u64) -> u64 {
    match std::env::var(name) {
        Ok(value) => value
            .parse::<u64>()
            .ok()
            .filter(|n| *n > 0 && *n <= maximum)
            .unwrap_or_else(|| panic!("{name} must be in 1..={maximum}")),
        Err(std::env::VarError::NotPresent) => default,
        Err(error) => panic!("{name}: {error}"),
    }
}
pub fn next(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed
}
/// UB/overflow/zero divisionを作らないbounded grammar。独立host計算がoracle。
pub fn generated(seed: u64, count: usize) -> Vec<Case> {
    let mut state = seed;
    (0..count)
        .map(|index| {
            let input = GeneratedInput {
                variant: index % 18,
                a: (next(&mut state) % 2001) as i64 - 1000,
                b: (next(&mut state) % 2001) as i64 - 1000,
                c: (next(&mut state) % 97 + 1) as i64,
            };
            generated_case(format!("generated_{index}"), seed, input)
        })
        .collect()
}
fn generated_case(name: String, seed: u64, input: GeneratedInput) -> Case {
    let (a, b, c) = (input.a, input.b, input.c);
    let (body, expected) = match input.variant {
            0 => (format!("    return ({a} + {b}) * {c} - ({a} - {b})\n"), (a+b)*c-(a-b)),
            1 => (format!("    return ({a} * {b}) / {c}\n"), a*b/c),
            2 => (format!("    values = [{a}, {b}, {c}]\n    return values[0] + len(values) * values[2]\n"), a+3*c),
            3 => (format!("    if {a} < {b} and not ({c} == 0):\n        return {a}\n    else:\n        return {b}\n"), a.min(b)),
            4 => (format!("    total = {a}\n    for value in range(4):\n        total += value\n    return total + {b}\n"), a+b+6),
            5 => (format!("    text = \"Nagi日本語\"\n    borrowed = view(text)\n    duplicate = copy(borrowed)\n    return len(duplicate) * {c} + {a}\n"), 13*c+a),
            6 => (format!("    text = \"Nagi日本語\"\n    other = \"abc\"\n    alias = view(text)\n    if {a} < {b}:\n        alias = view(other)\n    return len(alias) + {c}\n"), if a < b { 3+c } else { 13+c }),
            7 => (format!("    text = \"Nagi日本語\"\n    alias = view(text)\n    for index in range(3):\n        local = \"inner\"\n        alias = view(local)\n        alias = view(text)\n    return len(alias) + {a}\n"), 13+a),
            8 => (format!("    outer: Result[Result[i64, i64], i64] = ok(ok({a}))\n    match outer:\n        case Ok(inner):\n            match inner:\n                case Ok(value):\n                    return value + {b}\n                case Err(code):\n                    return code\n        case Err(problem):\n            return problem\n"), a+b),
            9 => (format!("    text = \"Nagi日本語\"\n    values = [view(text)]\n    moved = move(values)\n    values = [view(text)]\n    return len(moved[0]) + len(values[0]) + {a}\n"), 26+a),
            10 => (format!("    text = \"Nagi日本語\"\n    reader = identity\n    alias = reader(view(text))\n    return len(alias) + {a}\n"), 13+a),
            13 => (format!("    value = \"Nagi日本語\"\n    transferred = move(value)\n    value = \"abc\"\n    return len(transferred) + len(value) + {a}\n"), 16+a),
            14 => (format!("    value = [{a}, {b}]\n    transferred = move(value)\n    return transferred[0] + transferred[1]\n"), a+b),
            15 => (format!("    value: Result[Option[i64], i64] = ok(some({a}))\n    transferred = move(value)\n    match transferred:\n        case Ok(optional):\n            match optional:\n                case Some(number):\n                    return number + {b}\n                case None:\n                    return 0\n        case Err(problem):\n            return problem\n"), a+b),
            16 => (format!("    value = \"Nagi日本語\"\n    if {a} < {b}:\n        transferred = move(value)\n        return len(transferred) + {c}\n    else:\n        transferred = move(value)\n        return len(transferred) + {a}\n"), if a < b {13+c} else {13+a}),
            17 => (format!("    value = \"Nagi日本語\"\n    total = {a}\n    for index in range(3):\n        transferred = move(value)\n        total += len(transferred)\n        value = \"abc\"\n    return total + len(value)\n"), a+22),
            _ => (format!("    text = \"Nagi日本語\"\n    other = \"abc\"\n    chosen = choose(view(text), view(other), {a} < {b})\n    match chosen:\n        case Ok(optional):\n            match optional:\n                case Some(alias):\n                    return len(alias) + {c}\n                case None:\n                    return 0\n        case Err(problem):\n            return problem\n"), if a < b { 13+c } else { 3+c }),
        };
    let helpers = match input.variant {
        10 => "def identity(text: view[str]) -> view[str]:\n    return text\n",
        11 => "def choose(left: view[str], right: view[str], first: bool) -> Result[Option[view[str]], i64]:\n    if first:\n        return ok(some(left))\n    return ok(some(right))\n",
        12 => "async def choose(left: view[str], right: view[str], first: bool) -> Result[Option[view[str]], i64]:\n    if first:\n        return ok(some(left))\n    return ok(some(right))\n",
        _ => "",
    };
    let (declaration, body, oracle) = if input.variant == 12 {
        ("async def", body.replace("chosen = choose(", "chosen = await choose("),
         format!("let mut future = ::std::pin::pin!(evaluate()); let mut context = ::std::task::Context::from_waker(::std::task::Waker::noop()); assert!(matches!(::std::future::Future::poll(future.as_mut(), &mut context), ::std::task::Poll::Ready(value) if value == {expected}i64));"))
    } else {
        (
            "def",
            body,
            format!("assert_eq!(evaluate(), {expected}i64);"),
        )
    };
    Case {
        name,
        source: format!("{}{helpers}{declaration} evaluate() -> i64:\n{body}", if input.variant == 9 || input.variant >= 13 { "from std.ownership import move\n" } else { "" }),
        high: true,
        expected: "run-pass".into(),
        diagnostic: String::new(),
        line: 0,
        oracle,
        seed,
        generator: Some(input),
    }
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        loop {
            let root = std::env::temp_dir().join(format!(
                "nagi-conformance-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ));
            match fs::create_dir(&root) {
                Ok(()) => return Self(root),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("conformance fixture: {error}"),
            }
        }
    }
}
fn resolve_imports(case: &Case, program: nagic::ast::Program) -> Result<nagic::ast::Program, Box<Failure>> {
    if program.module_imports.is_empty() {
        return Ok(program);
    }
    // canonical標準operationもCLIと同じ解決を通す。未解決を成功扱いしない。
    let fixture = Fixture::new();
    let path = fixture.0.join(if case.high { "main.nagi" } else { "main.low" });
    step(case, "resolve-imports", || {
        fs::write(&path, &case.source).map_err(|error| error.to_string())?;
        nagic::source::load(&path, case.high).map(|sources| sources.program)
    })
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
pub fn unit(index: usize, label: &str, rust: &str, oracle: &str) -> String {
    // emitはtop-levelの関数呼び出しをcrate::で出力する。batchの各moduleへ
    // rootだけを移す。通常のRust string literal内部（Nagiの出力）は変更しない。
    let root = format!("crate::case_{index}_{label}::");
    let mut relocated = String::new();
    let mut at = 0;
    while at < rust.len() {
        let rest = &rust[at..];
        let literal_end = if rest.starts_with("//") {
            Some(rest.find('\n').unwrap_or(rest.len()))
        } else if rest.starts_with("/*") {
            let mut end = 2;
            let mut depth = 1;
            while end < rest.len() && depth != 0 {
                if rest[end..].starts_with("/*") {
                    depth += 1;
                    end += 2;
                } else if rest[end..].starts_with("*/") {
                    depth -= 1;
                    end += 2;
                } else {
                    end += rest[end..].chars().next().unwrap().len_utf8();
                }
            }
            Some(end)
        } else {
            let raw = rest.strip_prefix("br").or_else(|| rest.strip_prefix('r'));
            let raw_end = raw.and_then(|tail| {
                let hashes = tail.chars().take_while(|ch| *ch == '#').count();
                if tail.as_bytes().get(hashes) != Some(&b'"') {
                    return None;
                }
                let prefix = rest.len() - tail.len() + hashes + 1;
                let closing = format!("\"{}", "#".repeat(hashes));
                Some(
                    rest[prefix..]
                        .find(&closing)
                        .map(|end| prefix + end + closing.len())
                        .unwrap_or(rest.len()),
                )
            });
            raw_end.or_else(|| {
                if !rest.starts_with('"') {
                    return None;
                }
                let mut escaped = false;
                for (index, ch) in rest.char_indices().skip(1) {
                    if escaped {
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == '"' {
                        return Some(index + 1);
                    }
                }
                Some(rest.len())
            })
        };
        if let Some(end) = literal_end {
            relocated.push_str(&rest[..end]);
            at += end;
            continue;
        }
        let boundary = at == 0
            || !rust[..at]
                .chars()
                .next_back()
                .is_some_and(|ch| ch.is_alphanumeric() || ch == '_' || ch == '#');
        if boundary && rest.starts_with("crate::") {
            relocated.push_str(&root);
            at += "crate::".len();
            continue;
        }
        let ch = rest.chars().next().unwrap();
        relocated.push(ch);
        at += ch.len_utf8();
    }
    format!("mod case_{index}_{label} {{\n{relocated}\n#[test] fn contract() {{ {oracle} }}\n}}\n")
}
fn compile(code: &str, run: bool) -> Result<(), (String, String)> {
    let fixture = Fixture::new();
    let file = fixture.0.join("batch.rs");
    let binary = fixture
        .0
        .join(format!("batch{}", std::env::consts::EXE_SUFFIX));
    fs::write(&file, code).map_err(|e| ("rustc".into(), e.to_string()))?;
    let mut rustc = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    rustc
        .args(["--edition=2021", "--test", "-D", "unused-imports"])
        .arg(&file)
        .arg("-o")
        .arg(&binary);
    bounded_process(&mut rustc, "rustc", Duration::from_secs(30), &fixture.0)?;
    if run {
        let mut native = Command::new(binary);
        native.args(["--test-threads=1", "--nocapture"]);
        bounded_process(&mut native, "runtime", Duration::from_secs(10), &fixture.0)?;
    }
    Ok(())
}
pub fn bounded_process(
    command: &mut Command,
    stage: &str,
    deadline: Duration,
    root: &std::path::Path,
) -> Result<(), (String, String)> {
    // pipe bufferでdeadlockしないよう出力は小fixture内へ直接書く。
    let stdout = root.join(format!("{stage}.stdout"));
    let stderr = root.join(format!("{stage}.stderr"));
    command
        .stdout(Stdio::from(
            fs::File::create(&stdout).map_err(|e| (stage.into(), e.to_string()))?,
        ))
        .stderr(Stdio::from(
            fs::File::create(&stderr).map_err(|e| (stage.into(), e.to_string()))?,
        ));
    let mut child = command.spawn().map_err(|e| (stage.into(), e.to_string()))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(_)) => {
                return Err((
                    stage.into(),
                    format!(
                        "{}{}",
                        fs::read_to_string(&stdout).unwrap_or_default(),
                        fs::read_to_string(&stderr).unwrap_or_default()
                    ),
                ))
            }
            Ok(None) if started.elapsed() < deadline => {
                std::thread::sleep(Duration::from_millis(10))
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err((
                    format!("{stage}-timeout"),
                    format!(
                        "deadline={}s wait={result:?}\n{}{}",
                        deadline.as_secs(),
                        fs::read_to_string(&stdout).unwrap_or_default(),
                        fs::read_to_string(&stderr).unwrap_or_default()
                    ),
                ));
            }
        }
    }
}
pub fn failure_case(failure: &Failure) -> Case {
    Case {
        name: failure.name.clone(),
        source: failure.source.clone(),
        seed: failure.seed,
        expected: failure.expected.clone(),
        high: failure.high,
        oracle: failure.oracle.clone(),
        diagnostic: failure.expected_diagnostic.clone(),
        line: failure.expected_line,
        generator: failure.generator.clone(),
    }
}

fn isolated(case: &Case) -> Result<(), Box<Failure>> {
    let (direct, saved) = pipeline(case)?;
    let code = format!(
        "{}{}",
        unit(0, "direct", &direct, &case.oracle),
        unit(0, "saved", &saved, &case.oracle)
    );
    compile(&code, case.expected == "run-pass")
        .map_err(|(stage, error)| Failure::new(case, &stage, error))
}

// A rustc rejection used for source shrinking must come from generated code,
// not from an oracle referring to a declaration that shrinking removed.
fn backend_without_oracle(case: &Case) -> Result<(), Box<Failure>> {
    let (direct, saved) = pipeline(case)?;
    let code = format!(
        "{}{}",
        unit(0, "direct", &direct, ""),
        unit(0, "saved", &saved, "")
    );
    compile(&code, false).map_err(|(stage, error)| Failure::new(case, &stage, error))
}

/// 正常経路は全caseを1 rustcに束ねる。失敗時だけ個別再現し黙殺しない。
pub fn run_cases(cases: &[Case]) -> Result<(), Box<Failure>> {
    let mut batch = String::new();
    let mut positives = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        if case.expected.starts_with("reject:") {
            negative(case)?;
            continue;
        }
        assert!(matches!(
            case.expected.as_str(),
            "run-pass" | "compile-pass"
        ));
        let (direct, saved) = pipeline(case)?;
        batch.push_str(&unit(index, "direct", &direct, &case.oracle));
        batch.push_str(&unit(index, "saved", &saved, &case.oracle));
        positives.push(case);
    }
    if batch.is_empty() {
        return Ok(());
    }
    if let Err((stage, diagnostic)) = compile(&batch, true) {
        // timeoutは追加の個別compileで何度も再現せず、batchそのものを保存する。
        if !stage.ends_with("-timeout") {
            for case in positives.into_iter().take(32) {
                isolated(case)?;
            }
        }
        // 個別再現が取れなくてもbatch failureを失敗扱いし、wrong先頭caseでなく
        // 実際のRust batch sourceをartifactへ残す。
        let batch_case = Case {
            name: "batch".into(),
            source: batch,
            high: false,
            expected: "rust-batch-run-pass".into(),
            oracle: String::new(),
            diagnostic: String::new(),
            line: 0,
            seed: cases[0].seed,
            generator: None,
        };
        return Err(Failure::new(
            &batch_case,
            &format!("batch-{stage}"),
            diagnostic,
        ));
    }
    Ok(())
}

fn signature(failure: &Failure) -> String {
    if failure.stage == "runtime"
        && failure
            .diagnostic
            .contains("assertion `left == right` failed")
    {
        return "runtime:assertion `left == right` failed".into();
    }
    if let Some(start) = failure.diagnostic.find("error[E") {
        return failure.diagnostic[start..]
            .split(']')
            .next()
            .unwrap()
            .to_owned();
    }
    // frontendはline位置変動を除き、診断内容を保持する。
    failure
        .diagnostic
        .strip_prefix("line ")
        .and_then(|s| s.split_once(':'))
        .map(|(_, message)| message.trim().to_owned())
        .unwrap_or_else(|| failure.diagnostic.clone())
}
/// UTF-8を保持したchunk削除。stageとdiagnostic signatureを保持するbounded delta reduction。
fn source_reduction(case: &Case, failure: &Failure, budget: usize) -> String {
    let mut source = case.source.clone();
    let mut granularity = 2;
    let mut attempts = 0;
    while source.chars().count() > 1 && attempts < budget {
        let positions: Vec<_> = source
            .char_indices()
            .map(|(at, _)| at)
            .chain(std::iter::once(source.len()))
            .collect();
        let count = positions.len() - 1;
        let chunk = count.div_ceil(granularity);
        let mut changed = false;
        for from in (0..count).step_by(chunk.max(1)) {
            if attempts >= budget {
                break;
            }
            attempts += 1;
            let until = (from + chunk).min(count);
            let candidate = format!(
                "{}{}",
                &source[..positions[from]],
                &source[positions[until]..]
            );
            // negativeのoracleは指定の拒否構文/行に結び付く。対象行を
            // 削除して普通のvalid programへ変える「縮小」は認めない。
            if case.expected.starts_with("reject:") {
                let target = case.source.lines().nth(case.line.saturating_sub(1));
                if candidate.lines().nth(case.line.saturating_sub(1)) != target {
                    continue;
                }
            }
            let mut reduced = case.clone();
            reduced.source = candidate.clone();
            let result = if case.expected == "initial-reject-or-checked-pipeline" {
                mutation(&reduced).map(|_| ())
            } else if case.expected.starts_with("reject:") {
                negative(&reduced)
            } else if failure.stage == "rustc" {
                backend_without_oracle(&reduced)
            } else {
                pipeline(&reduced).map(|_| ())
            };
            if let Err(error) = result {
                if error.stage == failure.stage && signature(&error) == signature(failure) {
                    source = candidate;
                    changed = true;
                    granularity = granularity.saturating_sub(1).max(2);
                    break;
                }
            }
        }
        if !changed {
            if granularity >= count {
                break;
            }
            granularity = (granularity * 2).min(count);
        }
    }
    source
}
fn minimized_case(case: &Case, failure: &Failure, budget: usize) -> Case {
    if failure.stage.starts_with("batch-") || failure.stage.ends_with("-timeout") {
        return case.clone();
    }
    if failure.stage != "runtime" {
        if failure.stage == "rustc" {
            match backend_without_oracle(case) {
                Err(error)
                    if error.stage == failure.stage && signature(&error) == signature(failure) => {}
                // Preserve the complete original when the rejection depends
                // on the harness. Do not label an empty program a compiler bug.
                _ => return case.clone(),
            }
        }
        let mut reduced = case.clone();
        reduced.source = source_reduction(case, failure, budget);
        return reduced;
    }
    // runtime oracleはsourceの意味に依存する。生成パラメータだけを縮め、
    // host期待値を再計算する。固定corpusのsourceを任意削除しない。
    let Some(mut input) = case.generator.clone() else {
        return case.clone();
    };
    let mut best = case.clone();
    let mut attempts = 0;
    for field in 0..3 {
        let current = match field {
            0 => input.a,
            1 => input.b,
            _ => input.c,
        };
        let candidates = [
            if field == 2 { 1 } else { 0 },
            if field == 2 {
                (current / 2).max(1)
            } else {
                current / 2
            },
        ];
        for value in candidates {
            if attempts >= budget {
                return best;
            }
            if value == current {
                continue;
            }
            attempts += 1;
            let mut next = input.clone();
            match field {
                0 => next.a = value,
                1 => next.b = value,
                _ => next.c = value,
            }
            let candidate = generated_case(case.name.clone(), case.seed, next.clone());
            if let Err(error) = isolated(&candidate) {
                if error.stage == failure.stage && signature(&error) == signature(failure) {
                    best = candidate;
                    input = next;
                    break;
                }
            }
        }
    }
    best
}
pub fn minimize(case: &Case, failure: &Failure, budget: usize) -> String {
    minimized_case(case, failure, budget).source
}

pub fn save_failure(case: &Case, failure: &Failure) -> PathBuf {
    let root = std::env::var_os("NAGI_FAILURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("nagi-conformance-failures"));
    fs::create_dir_all(&root).expect("failure artifact directory");
    let stem: String = case
        .name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stem = format!("{stem}-{:016x}-{}", case.seed, std::process::id());
    let path = root.join(format!("{stem}.json"));
    fs::write(&path, serde_json::to_vec_pretty(failure).unwrap()).expect("failure record");
    fs::write(root.join(format!("{stem}.source")), &case.source).expect("failure source");
    let budget = if matches!(failure.stage.as_str(), "rustc" | "runtime") {
        8
    } else {
        96
    };
    let reduced = minimized_case(case, failure, budget);
    fs::write(root.join(format!("{stem}.min.source")), &reduced.source).expect("minimized source");
    fs::write(
        root.join(format!("{stem}.min.case.json")),
        serde_json::to_vec_pretty(&reduced).unwrap(),
    )
    .expect("minimized case with recomputed oracle");
    path
}
