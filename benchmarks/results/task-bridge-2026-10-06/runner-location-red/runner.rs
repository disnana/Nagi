//! S1の先行contract runner。未実装の拒否をchecker成功へ読み替えない。
use nagic::{check, parser, source};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, process::ExitCode};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    source: String,
    high: bool,
    contract: String,
    expected: String,
    diagnostic: String,
    primary_line: usize,
}

#[derive(Debug, Serialize)]
struct Observation {
    case: Case,
    stage: String,
    diagnostic: String,
    line: Option<usize>,
    matched_contract: bool,
}

fn matches(case: &Case, stage: &str, diagnostic: &str, line: Option<usize>) -> bool {
    match case.expected.as_str() {
        "check-pass" => stage == "accepted",
        "check-fail" => {
            stage == "check"
                && diagnostic.contains(&case.diagnostic)
                && line == Some(case.primary_line)
        }
        _ => false,
    }
}

fn message_line(message: &str) -> Option<usize> {
    message
        .strip_prefix("line ")?
        .split(':')
        .next()?
        .parse()
        .ok()
}

fn observe(root: &Path, case: Case) -> Result<Observation, Box<dyn std::error::Error>> {
    let path = root.join(&case.source);
    let text = fs::read_to_string(&path)?;
    let (stage, diagnostic, line) = match parser::parse(&text, case.high) {
        Err(error) => ("parse", error.clone(), message_line(&error)),
        Ok(_) => match source::load(&path, case.high) {
            Err(error) => ("resolve", error, None),
            Ok(mut loaded) => match check::check(&mut loaded.program) {
                Ok(()) => ("accepted", String::new(), None),
                Err(error) => {
                    let line = message_line(&error)
                        .and_then(|global| loaded.location(global))
                        .filter(|location| location.path == path)
                        .map(|location| location.line);
                    ("check", loaded.diagnostic(&error), line)
                }
            },
        },
    };
    let matched_contract = matches(&case, stage, &diagnostic, line);
    Ok(Observation {
        case,
        stage: stage.into(),
        diagnostic,
        line,
        matched_contract,
    })
}

fn run() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/task-handles");
    let cases: Vec<Case> = serde_json::from_slice(&fs::read(root.join("contracts.json"))?)?;
    let mut observed = Vec::new();
    for case in cases {
        // ICEは期待したnegativeと異なる。panicを成功にしない。
        observed.push(observe(&root, case)?);
    }
    let passed = observed.iter().filter(|case| case.matched_contract).count();
    let report = serde_json::json!({
        "scope": "parse/name resolution/check only; no lowering, Rust build, runtime or static Task guarantee",
        "total": observed.len(), "matched": passed, "unmatched": observed.len() - passed,
        "observations": observed,
    });
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 || args[0] != "--report" {
        return Err("usage: task-contract-red --report <file.json>".into());
    }
    fs::write(&args[1], serde_json::to_vec_pretty(&report)?)?;
    println!(
        "Task contracts: {passed}/{} matched; report: {}",
        observed.len(),
        args[1]
    );
    if passed == observed.len() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("Task contract runner failed: {error}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invalid() -> Case {
        Case {
            name: "unreceived".into(),
            source: "case.nagi".into(),
            high: true,
            contract: "must-consume".into(),
            expected: "check-fail".into(),
            diagnostic: "未受取Task".into(),
            primary_line: 4,
        }
    }

    #[test]
    fn parse_or_resolve_rejection_cannot_satisfy_a_checker_contract() {
        for stage in ["parse", "resolve", "ice"] {
            assert!(!matches(&invalid(), stage, "未受取Task", Some(4)));
        }
        assert!(matches(&invalid(), "check", "未受取Task", Some(4)));
    }

    #[test]
    fn wrong_diagnostic_or_source_location_is_a_contract_failure() {
        assert!(!matches(&invalid(), "check", "型不一致", Some(4)));
        assert!(!matches(&invalid(), "check", "未受取Task", Some(5)));
        assert!(!matches(&invalid(), "accepted", "", None));
    }

    #[test]
    fn existing_checker_negative_preserves_original_fixture_location() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/task-handles");
        for (source, high) in [("checker-location-control.nagi", true), ("checker-location-control.low", false)] {
            let case = Case { name: source.into(), source: source.into(), high,
                contract: "infrastructure-control".into(), expected: "check-fail".into(),
                diagnostic: "型".into(), primary_line: 2 };
            let observed = observe(&root, case).unwrap();
            assert_eq!(observed.stage, "check");
            assert_eq!(observed.line, Some(2), "{observed:?}");
            assert!(observed.matched_contract, "{observed:?}");
        }
    }
}
