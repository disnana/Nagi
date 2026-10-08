//! SQLite公開契約の先行runner。parse/名前解決の拒否を負例の成功にしない。
use nagic::{check, parser, source};
use serde_json::{json, Value};
use std::{fs, path::Path, process::ExitCode};

fn message_line(message: &str) -> Option<usize> {
    message
        .strip_prefix("line ")?
        .split(':')
        .next()?
        .parse()
        .ok()
}

fn matches_contract(
    expected: &str,
    stage: &str,
    diagnostic: &str,
    fragment: &str,
    line: Option<usize>,
    primary_line: Option<usize>,
) -> bool {
    match expected {
        "pass" => stage == "accepted",
        "fail" => {
            stage == "check"
                && !fragment.is_empty()
                && diagnostic.contains(fragment)
                && primary_line.is_some()
                && line == primary_line
        }
        _ => false,
    }
}

fn observe(path: &Path, high: bool) -> Result<(String, String, Option<usize>), String> {
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(match parser::parse(&text, high) {
        Err(error) => ("parse".into(), error.clone(), message_line(&error)),
        Ok(_) => match source::load(&path, high) {
            Err(error) => ("resolve".into(), error, None),
            Ok(mut loaded) => match check::check(&mut loaded.program) {
                Ok(()) => ("accepted".into(), String::new(), None),
                Err(error) => {
                    let line = message_line(&error)
                        .and_then(|global| loaded.location(global))
                        .filter(|location| location.path == path)
                        .map(|location| location.line);
                    // Match the checker message only. Rendering adds a path and
                    // source excerpt that can contain the expected fragment even
                    // when the actual rejection has an unrelated cause.
                    ("check".into(), error, line)
                }
            },
        },
    })
}

fn run() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 || args[0] != "--report" {
        return Err("usage: sqlite-contract-check --report <file.json>".into());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sqlite-contract");
    let matrix: Value = serde_json::from_slice(&fs::read(root.join("matrix.json"))?)?;
    let cases = matrix["cases"].as_array().ok_or("missing cases")?;
    let mut observations = Vec::new();
    for case in cases {
        let id = case["id"].as_str().ok_or("missing ID")?;
        let expected = case["expected_after_public_vertical_slice"]
            .as_str()
            .ok_or("missing expected contract")?;
        let fragment = case["checker_diagnostic_fragment"]
            .as_str()
            .ok_or("missing diagnostic fragment")?;
        for (mode, high) in [("high", true), ("user_low", false)] {
            let name = case["inputs"][mode].as_str().ok_or("missing input")?;
            let primary_line = case["expected_origin"][mode]["line"]
                .as_u64()
                .and_then(|line| usize::try_from(line).ok());
            // A panic is an infrastructure/compiler failure, never a negative GREEN.
            let (stage, diagnostic, line) =
                match std::panic::catch_unwind(|| observe(&root.join(name), high)) {
                    Ok(result) => result?,
                    Err(_) => ("panic".into(), "compiler panicked".into(), None),
                };
            let matched =
                matches_contract(expected, &stage, &diagnostic, fragment, line, primary_line);
            observations.push(json!({
                "id": id, "source": name, "form": mode, "expected": expected,
                "stage": stage, "diagnostic": diagnostic, "primary_line": line,
                "expected_line": primary_line, "diagnostic_fragment": fragment,
                "matched_contract": matched,
                "saved_low": "not exercised by this checker runner",
                "native_build": "not exercised", "native_run": "not exercised",
            }));
        }
    }
    let matched = observations
        .iter()
        .filter(|o| o["matched_contract"] == true)
        .count();
    let report = json!({
        "scope": "High/handwritten Low parse, resolution, checker and original source line; saved Low/native require separate conformance",
        "total": observations.len(), "matched": matched,
        "unmatched": observations.len() - matched, "observations": observations,
    });
    fs::write(&args[1], serde_json::to_vec_pretty(&report)?)?;
    println!(
        "SQLite contracts: {matched}/{} matched; report: {}",
        cases.len() * 2,
        args[1]
    );
    Ok(if matched == cases.len() * 2 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("SQLite contract runner failed: {error}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infrastructure_failure_never_satisfies_a_checker_negative() {
        for stage in ["parse", "resolve", "panic", "accepted"] {
            assert!(!matches_contract(
                "fail",
                stage,
                "move後",
                "move後",
                Some(4),
                Some(4)
            ));
        }
        assert!(matches_contract(
            "fail",
            "check",
            "move後",
            "move後",
            Some(4),
            Some(4)
        ));
    }

    #[test]
    fn a_wrong_diagnostic_or_original_line_remains_red() {
        assert!(!matches_contract(
            "fail",
            "check",
            "型不一致",
            "move後",
            Some(4),
            Some(4)
        ));
        assert!(!matches_contract(
            "fail",
            "check",
            "move後",
            "move後",
            Some(5),
            Some(4)
        ));
        assert!(!matches_contract(
            "fail",
            "check",
            "move後",
            "",
            Some(4),
            Some(4)
        ));
        assert!(!matches_contract("pass", "check", "", "", None, None));
    }

    #[test]
    fn existing_checker_negative_is_mapped_to_the_original_source() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/task-handles");
        for (file, high) in [
            ("checker-location-control.nagi", true),
            ("checker-location-control.low", false),
        ] {
            let (stage, diagnostic, line) = observe(&root.join(file), high).unwrap();
            assert!(
                matches_contract("fail", &stage, &diagnostic, "型", line, Some(2)),
                "{file}: {stage}: {diagnostic}"
            );
        }
    }

    #[test]
    fn source_excerpt_is_not_checker_evidence() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/task-handles");
        for (file, high) in [
            ("checker-location-control.nagi", true),
            ("checker-location-control.low", false),
        ] {
            let (stage, diagnostic, line) = observe(&root.join(file), high).unwrap();
            // `wrong` appears in the source value, not in the type diagnostic.
            assert!(!matches_contract(
                "fail",
                &stage,
                &diagnostic,
                "wrong",
                line,
                Some(2)
            ));
        }
    }
}
