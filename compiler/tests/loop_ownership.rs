#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, parser};
use std::{fs, process::Command};

static FIXTURE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

const CLASSES: &str =
    "class Person:\n    name: str\n    age: i64\nclass Group:\n    person: Person\n";
const PRELUDE: &str = "def take(text: str):\n    print(text)\ndef ready(text: str) -> bool:\n    print(text)\n    return False\ndef count(text: str) -> i64:\n    print(text)\n    return 2\n";

fn checked(source: &str) -> Result<nagic::ast::Program, String> {
    let mut p = parser::parse(source, true)?;
    check::check(&mut p)?;
    Ok(p)
}

fn rejects(body: &str, place: &str) {
    let source = format!("{CLASSES}{PRELUDE}def main():\n{body}");
    let error = checked(&source).unwrap_err();
    assert!(error.contains("move後") && error.contains(place), "{error}");
}

fn accepts(body: &str) {
    let source = format!("{PRELUDE}def main():\n{body}");
    let high = checked(&source).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    // Also ask rustc to check the emitted ownership rules and declarations.
    // These programs use std only; metadata checking needs no linker or Cargo.
    let root = std::env::temp_dir().join(format!(
        "nagi-loop-{}-{}",
        std::process::id(),
        FIXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    let file = root.join("generated.rs");
    fs::write(&file, emit::rust(&checked_emission::seal(&low)).unwrap()).unwrap();
    let result = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2021", "--crate-type=lib", "--emit=metadata"])
        .arg(&file)
        .arg("-o")
        .arg(root.join("checked.rmeta"))
        .output()
        .unwrap();
    let _ = fs::remove_dir_all(&root);
    assert!(
        result.status.success(),
        "{source}\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn accepts_with_classes(body: &str) {
    let source = format!("{CLASSES}{PRELUDE}def main():\n{body}");
    let high = checked(&source).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
}

#[test]
fn owned_values_cannot_be_consumed_on_later_iterations() {
    for body in [
        "    name = \"Nagi\"\n    for number in range(2):\n        take(name)\n",
        "    name = \"Nagi\"\n    count = 0\n    while count < 2:\n        take(name)\n        count += 1\n",
        "    name = \"Nagi\"\n    for number in range(2):\n        wrapped = some(name)\n",
    ] {
        rejects(body, "name");
    }
    let source = "def take(values: List[i64]):\n    print(len(values))\ndef main():\n    values = [1, 2]\n    for number in range(2):\n        take(values)\n";
    assert!(checked(source).unwrap_err().contains("values はmove後"));
}

#[test]
fn fields_and_nested_fields_cannot_be_consumed_on_later_iterations() {
    rejects("    person = Person(name=\"Nagi\", age=1)\n    for number in range(2):\n        take(person.name)\n", "person.name");
    rejects("    group = Group(person=Person(name=\"Nagi\", age=1))\n    for number in range(2):\n        name = group.person.name\n", "group.person.name");
}

#[test]
fn while_conditions_are_checked_on_each_evaluation_and_on_exit() {
    rejects(
        "    name = \"Nagi\"\n    while ready(name):\n        print(1)\n",
        "name",
    );
    rejects(
        "    name = \"Nagi\"\n    while ready(name):\n        name = \"again\"\n    print(name)\n",
        "name",
    );
    accepts("    name = \"Nagi\"\n    while ready(name):\n        name = \"again\"\n");
}

#[test]
fn conditional_reinitialization_needs_every_continuing_path() {
    rejects("    name = \"Nagi\"\n    for number in range(2):\n        take(name)\n        if number == 0:\n            name = \"again\"\n", "name");
    rejects("    name = \"Nagi\"\n    for number in range(2):\n        take(name)\n        result: Result[i64, i64] = ok(number)\n        match result:\n            case Ok(_):\n                name = \"again\"\n            case Err(_):\n                print(0)\n", "name");
}

#[test]
fn reinitialization_before_and_after_a_move_is_allowed() {
    accepts("    name = \"Nagi\"\n    for number in range(2):\n        take(name)\n        name = \"again\"\n    print(name)\n");
    accepts("    name = \"Nagi\"\n    take(name)\n    for number in range(2):\n        name = \"again\"\n        take(name)\n");
    accepts_with_classes("    person = Person(name=\"Nagi\", age=1)\n    for number in range(2):\n        take(person.name)\n        person = Person(name=\"again\", age=2)\n    print(person.name)\n");
}

#[test]
fn all_continuing_branches_can_restore_a_moved_value() {
    accepts("    name = \"Nagi\"\n    for number in range(2):\n        take(name)\n        if number == 0:\n            name = \"first\"\n        else:\n            name = \"next\"\n    print(name)\n");
    accepts("    name = \"Nagi\"\n    for number in range(2):\n        take(name)\n        result: Result[i64, i64] = ok(number)\n        match result:\n            case Ok(_):\n                name = \"ok\"\n            case Err(_):\n                name = \"err\"\n    print(name)\n");
}

#[test]
fn return_paths_do_not_repeat_or_consume_values_on_other_paths() {
    accepts("    name = \"Nagi\"\n    while ready(name):\n        return\n");
    accepts("    name = \"Nagi\"\n    for number in range(2):\n        take(name)\n        return\n    print(name)\n");
    accepts("    name = \"Nagi\"\n    for number in range(2):\n        if number == 0:\n            take(name)\n            return\n        print(name)\n    print(name)\n");
    accepts("    name = \"Nagi\"\n    for number in range(2):\n        take(name)\n        if number == 0:\n            return\n        else:\n            name = \"again\"\n    print(name)\n");
    accepts("    name = \"Nagi\"\n    for number in range(2):\n        take(name)\n        result: Result[i64, i64] = ok(number)\n        match result:\n            case Ok(_):\n                return\n            case Err(_):\n                name = \"again\"\n    print(name)\n");
}

#[test]
fn a_for_iterator_expression_is_evaluated_only_once() {
    accepts("    name = \"Nagi\"\n    for number in range(count(name)):\n        print(number)\n");
}

#[test]
fn branch_joins_preserve_reassignment_and_return_outside_loops() {
    accepts("    name = \"Nagi\"\n    take(name)\n    if True:\n        name = \"first\"\n    else:\n        name = \"next\"\n    print(name)\n");
    accepts(
        "    name = \"Nagi\"\n    if True:\n        take(name)\n        return\n    print(name)\n",
    );
    rejects("    name = \"Nagi\"\n    take(name)\n    if True:\n        name = \"again\"\n    print(name)\n", "name");
}

#[test]
fn locals_copy_values_and_borrowed_arguments_are_available_each_iteration() {
    accepts("    for number in range(2):\n        name = \"Nagi\"\n        take(name)\n");
    accepts_with_classes("    person = Person(name=\"Nagi\", age=1)\n    for number in range(2):\n        print(person.age)\n        take(copy(view(person.name)))\n    print(person.name)\n");
    let source = "def read(text: view[str]):\n    print(text)\ndef main():\n    text = \"Nagi\"\n    for number in range(2):\n        read(view(text))\n    print(text)\n";
    checked(source).unwrap();
}

#[test]
fn zero_iterations_and_loop_bindings_preserve_outer_state() {
    rejects("    name = \"Nagi\"\n    take(name)\n    for number in range(0):\n        name = \"again\"\n    print(name)\n", "name");
    accepts(
        "    name = \"Nagi\"\n    for name in range(2):\n        print(name)\n    print(name)\n",
    );
    let source = format!("{PRELUDE}def choose(name: str, count: i64):\n    for number in range(count):\n        take(name)\n        return\n    print(name)\n");
    checked(&source).unwrap();
}

#[test]
fn nested_loops_are_checked_without_carrying_local_declarations() {
    rejects("    name = \"Nagi\"\n    for outer in range(2):\n        for inner in range(2):\n            take(name)\n", "name");
    accepts("    for outer in range(2):\n        name = \"Nagi\"\n        for inner in range(2):\n            take(name)\n            name = \"again\"\n        print(name)\n");
}

#[test]
fn standalone_low_rejects_repeated_moves_at_the_original_line() {
    let source = "fn take(text: str) -> unit { print(text); }\nfn main() -> unit {\n    let name: str = \"Nagi\";\n    for number in range(2) {\n        take(name);\n    }\n}\n";
    let mut low = parser::parse(source, false).unwrap();
    let error = check::check(&mut low).unwrap_err();
    assert!(error.starts_with("line 5:"), "{error}");
    assert!(error.contains("name はmove後"), "{error}");
}
