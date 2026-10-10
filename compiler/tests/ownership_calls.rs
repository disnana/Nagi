#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, parser, source};
use std::{fs, path::PathBuf, process::Command};

fn checked(text: &str, high: bool) -> Result<nagic::ast::Program, String> {
    let mut program = parser::parse(text, high)?;
    check::check(&mut program)?;
    Ok(program)
}

fn accepts(text: &str) {
    let high = checked(text, true).unwrap_or_else(|error| panic!("{text}\n{error}"));
    let low = emit::low(&high);
    let independent = checked(&low, false).unwrap_or_else(|error| panic!("{low}\n{error}"));
    emit::rust(&checked_emission::seal(&independent)).unwrap();
}

fn rejects(text: &str, high: bool, line: usize, reason: &str) {
    let error = checked(text, high).expect_err(text);
    assert!(error.starts_with(&format!("line {line}:")), "{error}");
    assert!(error.contains(reason), "{error}");
}

#[test]
fn earlier_view_arguments_block_later_moves_in_direct_and_aliased_calls() {
    for callee in ["use_text", "selected"] {
        let text = format!("def use_text(part: view[str], owned: str):\n    print(part)\ndef main():\n    selected = use_text\n    text = \"Nagi\"\n    {callee}(view(text), text)\n");
        rejects(&text, true, 6, "同じ式で先に参照");
    }
    rejects("fn use_text(part: view[str], owned: str) -> unit { print(part); }\nfn main() -> unit {\n    let text: str = \"Nagi\";\n    use_text(view(text), text);\n}\n", false, 4, "同じ式で先に参照");
    rejects("def use_text(owned: str, part: view[str]):\n    print(part)\ndef main():\n    text = \"Nagi\"\n    use_text(text, view(text))\n", true, 5, "move後");
}

#[test]
fn earlier_views_remain_borrowed_while_nested_later_arguments_are_evaluated() {
    rejects("def identity(part: view[str]) -> view[str]:\n    return part\ndef consume(text: str) -> i64:\n    return len(text)\ndef use_text(part: view[str], number: i64):\n    print(part)\ndef main():\n    text = \"Nagi\"\n    use_text(identity(view(text)), consume(text))\n", true, 9, "同じ式で先に参照");
    rejects("def use_many(parts: List[view[str]], owned: str):\n    print(parts[0])\ndef main():\n    text = \"Nagi\"\n    use_many([view(text)], text)\n", true, 5, "同じ式で先に参照");
    rejects("def use_optional(part: Option[view[str]], owned: str):\n    print(0)\ndef main():\n    text = \"Nagi\"\n    use_optional(some(view(text)), text)\n", true, 5, "同じ式で先に参照");
}

#[test]
fn earlier_list_elements_hold_views_through_later_element_evaluation() {
    rejects("def main():\n    text = \"Nagi\"\n    items: List[Result[view[str], str]] = [ok(view(text)), fail(text)]\n", true, 3, "同じ式で先に参照");
    rejects("fn main() -> unit {\n    let text: str = \"Nagi\";\n    let items: List[Result[view[str], str]] = [ok(view(text)), fail(text)];\n}\n", false, 3, "同じ式で先に参照");
    rejects("def use_many(items: List[Result[view[str], str]]):\n    print(len(items))\ndef main():\n    text = \"Nagi\"\n    use_many([ok(view(text)), fail(text)])\n", true, 5, "同じ式で先に参照");
    accepts("def main():\n    text = \"Nagi\"\n    other = \"other\"\n    items: List[Result[view[str], str]] = [ok(view(text)), fail(other)]\n    print(text)\n");
    accepts("def main():\n    text = \"Nagi\"\n    items: List[Result[i64, str]] = [ok(len(view(text))), fail(text)]\n    print(len(items))\n");
}

#[test]
fn completed_inner_calls_copies_and_disjoint_owners_do_not_keep_call_loans() {
    accepts("def use_text(part: view[str], owned: str):\n    print(part)\ndef use_length(number: i64, owned: str):\n    print(number)\ndef use_owned(first: str, second: str):\n    print(first)\ndef main():\n    first = \"first\"\n    other = \"other\"\n    use_text(view(first), other)\n    use_length(len(view(first)), first)\n    text = \"copy\"\n    use_owned(copy(view(text)), text)\n    last = \"last\"\n    use_text(view(last), \"temporary\")\n    use_owned(last, \"after call\")\n");
    accepts("class Pair:\n    first: str\n    second: str\ndef use_text(part: view[str], owned: str):\n    print(part)\ndef main():\n    pair = Pair(first=\"first\", second=\"second\")\n    use_text(view(pair.first), pair.second)\n    print(pair.first)\n");
}

#[test]
fn list_mutation_checks_outer_loans_without_rejecting_two_phase_reads() {
    rejects("def use_values(values: view[i64], ignored: unit):\n    print(len(values))\ndef main():\n    values = [1, 2]\n    use_values(view(values), append(values, 3))\n", true, 5, "参照");
    rejects("def consume(values: List[i64]) -> i64:\n    return len(values)\ndef main():\n    values = [1, 2]\n    append(values, consume(values))\n", true, 5, "同じ式で先に参照");
    rejects(
        "def main():\n    values: List[unit] = []\n    append(values, append(values, print(0)))\n",
        true,
        3,
        "参照",
    );
    accepts("def main():\n    values = [1, 2]\n    other = [3]\n    append(values, len(view(values)))\n    append(values, len(view(other)))\n    print(len(values))\n");
}

#[test]
fn discarded_owned_places_are_consumed_but_copy_and_borrowed_values_remain_available() {
    rejects(
        "def main():\n    text = \"Nagi\"\n    text\n    print(text)\n",
        true,
        4,
        "move後",
    );
    rejects(
        "fn main() -> unit {\n    let text: str = \"Nagi\";\n    text;\n    print(text);\n}\n",
        false,
        4,
        "move後",
    );
    rejects("class Person:\n    name: str\n    age: i64\ndef main():\n    person = Person(name=\"Nagi\", age=1)\n    person.name\n    print(person.name)\n", true, 7, "person.name はmove後");
    rejects(
        "def main():\n    text = \"Nagi\"\n    for index in range(2):\n        text\n",
        true,
        4,
        "次の周回",
    );
    accepts("def main():\n    text = \"Nagi\"\n    number = 1\n    part = view(text)\n    number\n    number\n    part\n    part\n    print(text)\n    print(number)\n");
    accepts("class Person:\n    name: str\n    age: i64\ndef main():\n    person = Person(name=\"Nagi\", age=1)\n    person.name\n    print(person.age)\n");
}

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(text: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "nagi call ownership {} {}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("main.nagi"), text).unwrap();
        Self(root)
    }
    fn rejected(&self, line: usize) {
        let mut loaded = source::load(&self.0.join("main.nagi"), true).unwrap();
        let error = check::check(&mut loaded.program).expect_err("borrow/move conflict accepted");
        let error = loaded.diagnostic(&error);
        assert!(error.contains(&format!("main.nagi:{line}")), "{error}");
        assert!(error.contains("同じ式で先に参照"), "{error}");
    }
    fn accepts(&self) {
        let mut loaded = source::load(&self.0.join("main.nagi"), true).unwrap();
        check::check(&mut loaded.program).unwrap();
        let low = emit::low(&loaded.program);
        checked(&low, false).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn implicit_standard_references_hold_the_owner_through_later_arguments() {
    Fixture::new("import std.http.server as http\ndef name(request: http.Request) -> str:\n    return \"name\"\ndef inspect(request: http.Request) -> Result[unit, Error]:\n    result = try http.header(request, name(request))\n    return ok(print(0))\n").rejected(5);
    Fixture::new("import std.actor as worker\ndef consume(value: worker.Actor[i64, i64, str]) -> i64:\n    return 0\nasync def send(value: worker.Actor[i64, i64, str]) -> Result[unit, worker.CallError]:\n    result = try await worker.call(value, consume(value), 0, 1000)\n    return ok(print(0))\n").rejected(5);
    Fixture::new("import std.http.server as http\ndef name(request: view[http.Request]) -> str:\n    return copy(request.path)\ndef inspect(request: http.Request) -> Result[unit, Error]:\n    result = try http.header(request, name(view(request)))\n    print(request.path)\n    return ok(print(0))\n").accepts();
}

#[test]
fn sqlite_receiver_holds_its_loan_through_parameters_evaluation() {
    Fixture::new("import std.db.sqlite as sqlite\ndef consume(tx: sqlite.Tx) -> sqlite.Parameters:\n    return sqlite.parameters()\nasync def remove(tx: sqlite.Tx) -> Result[unit, sqlite.Failure]:\n    count = try await sqlite.exec(tx, sqlite.literal(\"DELETE FROM rows\"), consume(tx))\n    return ok(print(count))\n").rejected(5);
    Fixture::new("import std.db.sqlite as sqlite\nasync def insert(tx: view[sqlite.Tx]) -> Result[unit, sqlite.Failure]:\n    text = \"owned payload\"\n    count = try await sqlite.exec(tx, sqlite.literal(\"INSERT INTO rows VALUES (?)\"), sqlite.bind_text(sqlite.parameters(), text))\n    return ok(print(count))\n").accepts();
}

#[test]
fn call_conflicts_and_discarded_moves_fail_before_backend_tools_are_needed() {
    for (text, line) in [
        ("def use_text(part: view[str], owned: str):\n    print(part)\ndef main():\n    text = \"Nagi\"\n    use_text(view(text), text)\n", 5),
        ("def main():\n    text = \"Nagi\"\n    text\n    print(text)\n", 4),
    ] {
        let fixture = Fixture::new(text);
        for command in ["check", "build"] {
            let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
                .current_dir(&fixture.0)
                .args([command, "main.nagi", "--no-project"])
                .env("PATH", "")
                .env("NAGI_ROOT", fixture.0.join("missing-runtime"))
                .output().unwrap();
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "{command} accepted {text}");
            assert!(error.contains(&format!("main.nagi:{line}")), "{error}");
            assert!(!error.contains("Rust backend") && !error.contains("Cargo"), "{error}");
            assert!(!fixture.0.join("build/main/src/main.rs").exists());
        }
    }
}
