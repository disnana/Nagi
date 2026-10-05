// Compare the same High/Low frontend workload before and after sealing.
// Compile with --cfg baseline against the pre-Phase-1 nagic library.
use nagic::{check, emit, parser};
use std::{hint::black_box, time::Instant};

fn source(width: usize) -> String {
    let mut text = String::from("class Point:\n    value: i64\n");
    for i in 0..width {
        text.push_str(&format!(
            "def restore_{i}(part: view[str]) -> List[view[str]]:\n    parts = [part]\n    local = \"temporary\"\n    parts = [view(local)]\n    parts = [part]\n    return parts\n\ndef select_{i}(input: Result[Result[str, i64], i64]) -> i64:\n    match input:\n        case Ok(value):\n            match value:\n                case Ok(text):\n                    return len(view(text))\n                case Err(code):\n                    return code\n        case Err(code):\n            return code\n"
        ));
    }
    text
}

fn pipeline(source: &str) -> (String, String) {
    let mut primary = parser::parse(source, true).unwrap();
    check::check(&mut primary).unwrap();
    let low_text = emit::low(&primary);
    let low = parser::parse(&low_text, false).unwrap();
    #[cfg(baseline)]
    let rust = {
        let mut low = low;
        check::check(&mut low).unwrap();
        emit::rust(&low).unwrap()
    };
    #[cfg(not(baseline))]
    let rust = {
        let checked = check::finalize(
            low,
            nagic::ast::Program::default(),
            nagic::source::SourceProvenance::user_low_unmapped(),
        )
        .unwrap();
        emit::rust(&checked).unwrap()
    };
    (low_text, rust)
}

fn main() {
    let output = std::env::args().nth(1).expect("output directory");
    std::fs::create_dir_all(&output).unwrap();
    for width in [1, 16, 64] {
        let source = source(width);
        let (low, rust) = pipeline(&source);
        std::fs::write(format!("{output}/{width}.nagi"), &source).unwrap();
        std::fs::write(format!("{output}/{width}.low"), &low).unwrap();
        std::fs::write(format!("{output}/{width}.rs"), &rust).unwrap();
        for _ in 0..10 {
            black_box(pipeline(&source));
        }
        let mut samples = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            for _ in 0..100 {
                black_box(pipeline(&source));
            }
            samples.push(start.elapsed().as_micros() as f64 / 100.0);
        }
        println!("{{\"width\":{width},\"source_bytes\":{},\"low_bytes\":{},\"rust_bytes\":{},\"microseconds\":{:?}}}", source.len(), low.len(), rust.len(), samples);
    }
}
