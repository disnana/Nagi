#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, parser, source};
use std::{fs, path::PathBuf, process::Command};
static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn checked(source: &str) -> Result<nagic::ast::Program, String> {
    struct Input(PathBuf);
    impl Drop for Input {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let input = loop {
        let path = std::env::temp_dir().join(format!(
            "nagi-iterator-check-{}-{}",
            std::process::id(),
            ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        match fs::create_dir(&path) {
            Ok(()) => break Input(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("exclusive iterator fixture: {error}"),
        }
    };
    let path = input.0.join("main.nagi");
    fs::write(&path, source).unwrap();
    let mut p = source::load(&path, true)?.program;
    check::check(&mut p)?;
    Ok(p)
}
fn accepts(source: &str) {
    let p = checked(source).unwrap();
    let mut low = parser::parse(&emit::low(&p), false).unwrap();
    check::check(&mut low).unwrap();
    let root = std::env::temp_dir().join(format!(
        "nagi-iterator-{}-{}",
        std::process::id(),
        ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("main.rs"),
        emit::rust(&checked_emission::seal(&low)).unwrap(),
    )
    .unwrap();
    let result = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2021", "--crate-type=lib", "--emit=metadata"])
        .arg(root.join("main.rs"))
        .arg("-o")
        .arg(root.join("out.rmeta"))
        .output()
        .unwrap();
    let _ = fs::remove_dir_all(&root);
    assert!(
        result.status.success(),
        "{source}\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn iterator_borrows_reject_mutation_reassignment_and_moves_on_continuing_paths() {
    for operation in [
        "append(values, item)",
        "values = [3, 4]",
        "taken = move(values)",
    ] {
        let import = if operation.contains("move(") {
            "from std.ownership import move\n"
        } else {
            ""
        };
        let source = format!(
            "{import}def main():\n    values = [1, 2]\n    for item in values:\n        {operation}\n"
        );
        let error = checked(&source).unwrap_err();
        assert!(
            // move caseだけcanonical importを1行追加。元operationを同じ位置で観測する。
            error.starts_with(&format!("line {}:", if import.is_empty() { 4 } else { 5 }))
                && error.contains("参照"),
            "{error}"
        );
    }
    let mut low = parser::parse("fn main() -> unit {\n    let values: List[i64] = [1, 2];\n    for item in values {\n        append(values, item);\n    }\n}\n", false).unwrap();
    assert!(check::check(&mut low).unwrap_err().starts_with("line 4:"));
}

#[test]
fn iterator_loans_survive_view_alias_changes_and_nested_loops() {
    for body in [
        "    alias = view(values)\n    for item in alias:\n        alias = view(other)\n        append(values, item)\n",
        "    for item in view(values):\n        for index in range(2):\n            append(values, item)\n",
    ] {
        let source = format!("def main():\n    values = [1, 2]\n    other = [3, 4]\n{body}");
        assert!(checked(&source).unwrap_err().contains("参照"));
    }
}

#[test]
fn unrelated_values_shadowed_bindings_and_post_loop_mutation_stay_valid() {
    for body in [
        "    for item in values:\n        append(other, item)\n    append(values, 3)\n",
        "    for values in values:\n        print(values)\n    append(values, 3)\n",
        "    alias = view(values)\n    for item in alias:\n        alias = view(other)\n        print(item)\n    alias = view(other)\n    append(values, 3)\n",
        "    for item in values:\n        for item in other:\n            print(item)\n    append(values, 3)\n",
        "    for item in copy(view(values)):\n        append(values, item)\n",
    ] {
        accepts(&format!("def main():\n    values = [1, 2]\n    other = [3, 4]\n{body}"));
    }
}

#[test]
fn returning_paths_do_not_retain_an_unused_iterator_loan() {
    accepts("def choose(values: List[i64]) -> List[i64]:\n    for item in values:\n        append(values, item)\n        return values\n    return values\n");
    accepts("def choose(values: List[i64]) -> List[i64]:\n    for item in values:\n        if item > 0:\n            append(values, item)\n            return values\n        print(item)\n    return values\n");
}

#[test]
fn field_loans_keep_disjoint_fields_available_and_block_the_whole_owner() {
    let base = "class Data:\n    values: List[i64]\n    name: str\ndef main():\n    data = Data(values=[1, 2], name=\"Nagi\")\n    alias = view(data.values)\n";
    checked(&format!(
        "{base}    name = data.name\n    print(len(alias))\n"
    ))
    .unwrap();
    for operation in [
        "taken = move(data)",
        "taken = data.values",
        "data = Data(values=[3], name=\"next\")",
    ] {
        let import = if operation.contains("move(") {
            "from std.ownership import move\n"
        } else {
            ""
        };
        assert!(checked(&format!("{import}{base}    {operation}\n"))
            .unwrap_err()
            .contains("参照"));
    }
}

#[test]
fn view_origins_survive_branch_joins_and_nonborrowed_call_arguments() {
    let source = "def select(label: i64, values: view[i64]) -> view[i64]:\n    return values\ndef main():\n    first = [1, 2]\n    second = [3, 4]\n    alias = select(0, view(first))\n    if True:\n        alias = view(second)\n    append(second, 5)\n    print(len(alias))\n";
    assert!(checked(source).unwrap_err().contains("参照"));
    assert!(
        checked(&source.replace("append(second, 5)", "append(first, 5)"))
            .unwrap_err()
            .contains("参照")
    );
}
