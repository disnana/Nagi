use nagic::{ast::S, check, emit, parser, source};
use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn checked(source: &str, high: bool) -> nagic::ast::Program {
    let mut program = parser::parse(source, high).unwrap();
    check::check(&mut program).unwrap_or_else(|error| panic!("{source}\n{error}"));
    program
}

fn rejects_both(source: &str, expected: &str) {
    let low = emit::low(&parser::parse(source, true).unwrap());
    for (code, high) in [(source, true), (low.as_str(), false)] {
        let mut program = parser::parse(code, high).unwrap();
        let line = program.functions[0].body[0].line;
        let error = check::check(&mut program).expect_err(code);
        assert!(error.contains(expected), "{code}\n{error}");
        assert!(
            error.starts_with(&format!("line {line}:")),
            "{code}\n{error}"
        );
    }
}

#[test]
fn string_views_reject_iteration_and_numeric_indexing_in_high_and_low() {
    rejects_both(
        "def consume(text: view[str]):\n    for part in text:\n        print(part)\n",
        "view[str]の文字列反復は未対応",
    );
    rejects_both(
        "def consume(text: view[str]):\n    print(text[0])\n",
        "indexには配列が必要",
    );
}

const HIGH: &str = r#"def sum_bytes(data: view[bytes]) -> i64:
    total = 0
    for byte in data:
        typed: u8 = byte
        total += i64(typed)
    return total
def first_byte(data: view[bytes]) -> u8:
    return data[0]
def zero_copies(data: view[bytes]) -> i64:
    total = 0
    for byte in data:
        byte = 0
        total += i64(byte)
    return total
def sum_chunks(chunks: view[view[bytes]]) -> i64:
    total = 0
    for chunk in chunks:
        for byte in chunk:
            total += i64(byte)
    return total
def count_wrapped_strings(values: view[owned[str]]) -> i64:
    total = 0
    for value in values:
        total += 1
    return total
def count_wrapped_bytes(values: view[owned[bytes]]) -> i64:
    total = 0
    for value in values:
        total += 1
    return total
def sum_list(values: List[i64]) -> i64:
    total = 0
    for value in values:
        total += value
    return total
"#;

#[test]
fn byte_views_use_u8_and_preserve_nested_views_and_owned_element_slices() {
    let high = checked(HIGH, true);
    let low = checked(&emit::low(&high), false);
    for program in [&high, &low] {
        let statement = &program.functions[0].body[1];
        assert!(matches!(statement.kind, S::For(..)));
        assert_eq!(statement.binding_type.as_ref().unwrap().0, "u8");
        assert!(!statement.binding_borrowed);
        for index in [4, 5] {
            let statement = &program.functions[index].body[1];
            assert_eq!(statement.binding_type.as_ref().unwrap().0, "owned");
            assert!(statement.binding_borrowed);
        }
    }

    let folder = std::env::temp_dir().join(format!(
        "nagi-view-iteration-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&folder).unwrap();
    for (name, program) in [("high", &high), ("saved-low", &low)] {
        let rust = folder.join(format!("{name}.rs"));
        let binary = folder.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        fs::write(&rust, format!("{}\n#[test] fn values() {{\nlet bytes = [0_u8, 128, 255];\nassert_eq!(sum_bytes(&bytes), 383);\nassert_eq!(first_byte(&bytes), 0);\nassert_eq!(zero_copies(&bytes), 0);\nassert_eq!(bytes, [0, 128, 255]);\nassert_eq!(sum_chunks(&[&bytes[..1], &bytes[1..]]), 383);\nassert_eq!(count_wrapped_strings(&[\"凪\".into(), \"hello\".into()]), 2);\nassert_eq!(count_wrapped_bytes(&[vec![1, 2], vec![3]]), 2);\nassert_eq!(sum_list(vec![1, 2, 3]), 6);\n}}\n", emit::rust(program).unwrap())).unwrap();
        let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2021", "--test"])
            .arg(&rust)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(binary).output().unwrap();
        assert!(
            output.status.success(),
            "{name}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::remove_dir_all(folder).unwrap();
}

#[test]
fn native_resource_views_still_reject_iteration() {
    let folder = std::env::temp_dir().join(format!(
        "nagi-resource-iteration-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&folder).unwrap();
    let file = folder.join("main.nagi");
    fs::write(&file, "import std.http.server as http\ndef consume(request: view[http.Request]):\n    for value in request:\n        print(0)\n").unwrap();
    let loaded = source::load(&file, true).unwrap();
    let mut program = loaded.program;
    let error = check::check(&mut program).unwrap_err();
    assert!(
        error.contains("resourceのviewは配列ではないためforで反復できません"),
        "{error}"
    );
    fs::remove_dir_all(folder).unwrap();
}
