// 固定seedのmutation/property smoke。coverage-guided fuzzerではない。
#[allow(dead_code)]
#[path = "../compiler/tests/support/conformance.rs"]
mod support;
use support::Case;

fn fatal(case: &Case, failure: &support::Failure) -> ! {
    let artifact = support::save_failure(case, failure);
    eprintln!(
        "{} stage={} expected={} seed={}\n{}\nartifact={}",
        case.name,
        failure.stage,
        failure.expected,
        failure.seed,
        failure.diagnostic,
        artifact.display()
    );
    std::process::exit(1);
}
fn main() {
    let seed = support::env_number("NAGI_FUZZ_SEED", 305419896, u64::MAX);
    let count = support::env_number("NAGI_FUZZ_MUTATIONS", 1000, 100000);
    let generated_count = support::env_number("NAGI_FUZZ_CASES", 16, 2048);
    let generated = support::generated(seed, generated_count as usize);
    // accepted bounded grammarはHigh/Low check・emit・rustc・native oracleまで必須。
    if let Err(failure) = support::run_cases(&generated) {
        fatal(&support::failure_case(&failure), &failure);
    }
    let corpus = [
        ("def evaluate() -> i64:\n    values = [7, 8]\n    return values[0] + len(values)\n", true),
        ("def evaluate(text: view[str]) -> str:\n    return copy(text)\n", true),
        ("fn evaluate() -> i64 { let values: List[i64] = [7, 8]; return values[0] + len(values); }", false),
        ("def evaluate() -> i64:\n    value: Result[i64, i64] = ok(7)\n    match value:\n        case Ok(number):\n            return number\n        case Err(code):\n            return code\n", true),
        ("from std.ownership import move\ndef evaluate() -> i64:\n    value = [7, 8]\n    transferred = move(value)\n    return transferred[0] + len(transferred)\n", true),
        ("from std.ownership import move;\nfn evaluate() -> i64 { let value: List[i64] = [7, 8]; let transferred: List[i64] = move(value); return transferred[0] + len(transferred); }", false),
    ];
    let mut state = seed;
    let mut accepted = 0;
    let mut parse_rejected = 0;
    let mut check_rejected = 0;
    for index in 0..count as usize {
        let value = support::next(&mut state);
        let (source, high) = corpus[index % corpus.len()];
        let mut bytes = source.as_bytes().to_vec();
        let at = value as usize % bytes.len();
        let edit = (value >> 32) as u8;
        match index % 3 {
            0 => {
                bytes.remove(at);
            }
            1 => bytes.insert(at, edit),
            _ => bytes[at] = edit,
        }
        let case = Case {
            name: format!("mutation_{index}"),
            source: String::from_utf8_lossy(&bytes).into(),
            high,
            expected: "initial-reject-or-checked-pipeline".into(),
            diagnostic: String::new(),
            line: 0,
            oracle: String::new(),
            seed,
            generator: None,
        };
        match support::mutation(&case) {
            Ok(support::MutationOutcome::Checked) => accepted += 1,
            Ok(support::MutationOutcome::ParseRejected) => parse_rejected += 1,
            Ok(support::MutationOutcome::CheckRejected) => check_rejected += 1,
            Err(failure) => fatal(&case, &failure),
        }
    }
    println!(
        "{}",
        serde_json::json!({
            "seed": seed, "text_mutations": count, "parse_rejected": parse_rejected, "check_rejected": check_rejected,
            "checked_low_emit_mutations": accepted, "bounded_native_cases": generated_count,
            "panics": 0, "coverage_guided": false
        })
    );
}
