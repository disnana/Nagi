use nagic::{ast::S, check, emit, lexer, parser, source};
use std::{fs, path::PathBuf, process::Command};

static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "nagi-low-multiline-{}-{}",
            std::process::id(),
            ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const HIGH_QUOTE: &str = r#"enum Adjustment:
    Flat(amount: i64)
class Quote:
    subtotal: i64
    adjustment: Adjustment
    label: str
def increment(value: i64) -> i64:
    return value + 1
def total(
    quote: Quote,
    adjust: fn[
        i64,
        i64
    ]
) -> i64:
    match quote.adjustment:
        case Adjustment.Flat(amount):
            return adjust(
                quote.subtotal + amount
            )
def answer() -> i64:
    prices = [
        10,
        (
            20
        )
    ]
    quote = Quote(
        subtotal=prices[
            0
        ] + prices[
            1
        ],
        # Closing symbols in comments do not end the constructor: )]
        adjustment=Adjustment.Flat(
            11
        ),
        label="([)] # text"
    )
    return total(
        quote,
        increment
    )
"#;

const LOW_QUOTE: &str = r#"enum Adjustment {
    Flat(amount: i64);
}
record Quote {
    subtotal: i64;
    adjustment: Adjustment;
    label: str;
}
fn increment(value: i64) -> i64 {
    return value + 1;
}
fn total(
    quote: Quote,
    adjust: fn[
        i64,
        i64
    ]
) -> i64 {
    match quote.adjustment {
        case Adjustment.Flat(amount) {
            return adjust(
                quote.subtotal + amount
            );
        }
    }
}
fn answer() -> i64 {
    let prices = [
        10,
        (
            20
        )
    ];
    let quote = Quote(
        subtotal=prices[
            0
        ] + prices[
            1
        ],
        # Closing symbols in comments do not end the constructor: )]
        adjustment=Adjustment.Flat(
            11
        ),
        label="([)] # text"
    );
    return total(
        quote,
        increment
    );
}
"#;

#[test]
fn multiline_quote_matches_high_and_runs_as_native_low() {
    let mut high = parser::parse(HIGH_QUOTE, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(LOW_QUOTE, false).unwrap();
    check::check(&mut low).unwrap();
    assert_eq!(emit::low(&low), emit::low(&high));

    let fixture = Fixture::new();
    let generated = fixture.0.join("generated.rs");
    let executable = fixture
        .0
        .join(if cfg!(windows) { "quote.exe" } else { "quote" });
    let mut rust = emit::rust(&low).unwrap();
    rust.push_str("\n#[test] fn calculates_quote() { assert_eq!(answer(), 42); }\n");
    fs::write(&generated, rust).unwrap();
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2021", "--test"])
        .arg(&generated)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(executable).output().unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn low_braces_keep_newline_statement_boundaries() {
    let mut program = parser::parse(
        "fn answer() -> i64 {\n    let total = 0\n    for value in [\n        10,\n        20\n    ] {\n        total += value\n    }\n    if total > 0 {\n        return total\n    } else {\n        return 0\n    }\n}\n",
        false,
    )
    .unwrap();
    check::check(&mut program).unwrap();
    let body = &program.functions[0].body;
    assert_eq!(body.len(), 3);
    assert!(matches!(body[1].kind, S::For(..)));
    assert!(matches!(body[2].kind, S::If(..)));
}

#[test]
fn multiline_errors_keep_physical_source_lines() {
    let fixture = Fixture::new();
    for (name, high, text) in [
        (
            "invalid.nagi",
            true,
            "def main():\n    print( # )] in comments has no effect\n        missing\n    )\n",
        ),
        (
            "invalid.low",
            false,
            "fn main() {\n    print( # )] in comments has no effect\n        missing\n    );\n}\n",
        ),
    ] {
        let path = fixture.0.join(name);
        fs::write(&path, text).unwrap();
        let mut loaded = source::load(&path, high).unwrap();
        let error = check::check(&mut loaded.program).unwrap_err();
        assert!(error.starts_with("line 3:"), "{error}");
        let diagnostic = loaded.diagnostic(&error);
        assert!(diagnostic.contains(&format!("{name}:3")), "{diagnostic}");
        assert!(diagnostic.contains("missing"), "{diagnostic}");
    }
    for high in [true, false] {
        let error = lexer::lex("print(\n    1\n))\n", high).unwrap_err();
        assert!(error.starts_with("line 3:2:"), "{error}");
        assert!(error.contains("対応する開き括弧がありません"), "{error}");
        assert!(lexer::lex("print(\n    [1]\n", high)
            .unwrap_err()
            .contains("EOF: 括弧が閉じていません"));
    }
}
