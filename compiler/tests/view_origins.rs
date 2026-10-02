use nagic::{check, emit, parser};
use std::{fs, path::PathBuf, process::Command};

fn checked(source: &str, high: bool) -> Result<nagic::ast::Program, String> {
    let mut program = parser::parse(source, high)?;
    check::check(&mut program)?;
    Ok(program)
}

fn accepts(source: &str) {
    let high = checked(source, true).unwrap_or_else(|error| panic!("{source}\n{error}"));
    checked(&emit::low(&high), false).unwrap();
}

#[test]
fn storing_views_of_temporary_owners_is_rejected_at_the_assignment() {
    for source in [
        "def main():\n    borrowed = view(\"Nagi\")\n    print(borrowed)\n",
        "def main():\n    borrowed = view([1, 2])\n    print(len(borrowed))\n",
        "def make() -> str:\n    return \"Nagi\"\ndef main():\n    borrowed = view(make())\n    print(borrowed)\n",
        "class Input:\n    text: str\ndef make() -> Input:\n    return Input(text=\"Nagi\")\ndef main():\n    borrowed = view(make().text)\n    print(borrowed)\n",
        "def identity(text: view[str]) -> view[str]:\n    return text\ndef main():\n    borrowed = identity(view(\"Nagi\"))\n    print(borrowed)\n",
        "def main():\n    borrowed = some(view(\"Nagi\"))\n",
        "def main():\n    borrowed = [view(\"Nagi\")]\n",
        "def main():\n    borrowed = [view(\"Nagi\")][0]\n    print(borrowed)\n",
        "def main() -> Result[unit, Error]:\n    borrowed = try slice(view(\"Nagi\"), 0, 2)\n    return ok(print(borrowed))\n",
        "fn main() -> unit {\n    let borrowed: view[str] = view(\"Nagi\");\n    print(borrowed);\n}\n",
    ] {
        let error = checked(source, source.starts_with("def") || source.starts_with("class"))
            .unwrap_err();
        assert!(error.contains("一時的な所有値") && error.contains("保存できません"), "{error}");
    }
}

#[test]
fn immediate_temporary_views_owned_copies_and_match_aliases_remain_legal() {
    for source in [
        "def read(text: view[str]):\n    print(text)\ndef main():\n    read(view(\"Nagi\"))\n    print(len(view([1, 2])))\n    owned = copy(view(\"Nagi\"))\n    print(owned)\n",
        "def main() -> Result[unit, Error]:\n    match slice(view(\"Nagi\"), 0, 2):\n        case Ok(part):\n            alias = part\n            print(alias)\n        case Err(_):\n            print(0)\n    return ok(print(\"done\"))\n",
        "def main() -> Result[unit, Error]:\n    owned = copy(try slice(view(\"Nagi\"), 0, 2))\n    return ok(print(owned))\n",
        "def main():\n    text = \"Nagi\"\n    owner = [view(text)]\n    borrowed = view(owner)\n    duplicate = copy(borrowed)\n    direct = copy(view([view(text)]))\n    print(len(duplicate))\n    print(len(direct))\n",
        "def main():\n    text = \"Nagi\"\n    borrowed = [view(text)][0]\n    duplicate = view([view(text)])[0]\n    print(borrowed)\n    print(duplicate)\n",
        "def main() -> Result[unit, Error]:\n    text = \"Nagi\"\n    duplicate = copy(try slice(view([view(text)]), 0, 1))\n    return ok(print(len(duplicate)))\n",
    ] {
        accepts(source);
    }
}

#[test]
fn copied_containers_cannot_keep_views_of_temporary_elements() {
    let source =
        "def main():\n    borrowed = copy(view([view(\"Nagi\")]))\n    print(len(borrowed))\n";
    assert!(checked(source, true)
        .unwrap_err()
        .contains("一時的な所有値"));
}

#[test]
fn copied_or_indexed_views_keep_only_the_content_owners() {
    let prefix = "def take(values: List[view[str]]):\n    print(len(values))\ndef main():\n    text = \"Nagi\"\n    other = \"other\"\n    values = [view(text)]\n";
    for body in [
        "    picked = view(values)[0]\n    take(values)\n    print(picked)\n",
        "    picked = view(values)[0]\n    values = [view(other)]\n    print(picked)\n",
        "    picked = copy(view(values))\n    take(values)\n    print(picked[0])\n",
        "    alias = view(values)\n    picked = alias[0]\n    replacement = [view(other)]\n    alias = view(replacement)\n    take(values)\n    print(picked)\n",
        "    picked = [view([view(text)])][0][0]\n    take(values)\n    print(picked)\n",
        "    outer = [view(values)]\n    picked = view(outer)[0][0]\n    outer = []\n    take(values)\n    print(picked)\n",
    ] {
        accepts(&format!("{prefix}{body}"));
    }
    for value in [
        "view(values)[0]",
        "copy(view(values))",
        "[view([view(text)])][0][0]",
    ] {
        let source = format!(
            "{prefix}    picked = {value}\n    values = []\n    text = \"changed\"\n    print(len(picked))\n"
        );
        assert!(checked(&source, true).unwrap_err().contains("参照中"));
    }
    let bad = "def main():\n    picked = [view([view(\"temporary\")])][0][0]\n    print(picked)\n";
    assert!(checked(bad, true).unwrap_err().contains("一時的な所有値"));
    accepts("def take(values: List[view[str]]):\n    print(len(values))\ndef project(values: List[view[str]]) -> view[str]:\n    picked = values[0]\n    take(values)\n    return picked\ndef main():\n    text = \"Nagi\"\n    picked = project([view(text)])\n    print(picked)\n");
    accepts("def take(values: List[view[str]]):\n    print(len(values))\ndef main():\n    text = \"Nagi\"\n    other = \"other\"\n    values = [view(text)]\n    replacement = [view(other)]\n    result = slice(view(values), 0, 1)\n    match result:\n        case Ok(parts):\n            picked = parts[0]\n            parts = view(replacement)\n            take(values)\n            print(picked)\n        case Err(_):\n            print(0)\n");
}

#[test]
fn returning_views_borrows_parameter_contents_without_borrowing_owned_containers() {
    for source in [
        "def bad(values: List[view[str]]) -> view[view[str]]:\n    return view(values)\n",
        "def bad(values: List[view[str]]) -> Result[view[view[str]], Error]:\n    return ok(view(values))\n",
        "def bad(values: List[view[str]]) -> List[view[view[str]]]:\n    return [view(values)]\n",
        "fn bad(values: List[view[str]]) -> view[view[str]] {\n    return view(values);\n}\n",
        "def bad(values: List[view[str]]) -> view[view[str]]:\n    return view([values[0]])\n",
        "def bad(values: List[view[str]]) -> Option[view[view[str]]]:\n    return some(view([values[0]]))\n",
        "def bad(values: List[view[str]]) -> List[view[view[str]]]:\n    return [view([values[0]])]\n",
        "fn bad(values: List[view[str]]) -> view[view[str]] {\n    return view([values[0]]);\n}\n",
    ] {
        let error = checked(source, source.starts_with("def")).unwrap_err();
        assert!(error.starts_with("line 2:") && error.contains("view escapes"), "{error}");
    }
    accepts("def copied(values: List[view[str]]) -> List[view[str]]:\n    return copy(view(values))\ndef first(values: List[view[str]]) -> view[str]:\n    return values[0]\ndef borrowed(values: view[view[str]]) -> view[view[str]]:\n    return values\n");
    accepts("def first(values: List[view[str]]) -> view[str]:\n    return view([values[0]])[0]\ndef copied_first(values: List[view[str]]) -> view[str]:\n    return view(copy(view(values)))[0]\n");
}

#[test]
fn borrowed_json_results_block_input_moves_and_reassignment() {
    for action in ["text = \"changed\"", "take(text)"] {
        let source = format!("def take(value: str):\n    print(value)\ndef main() -> Result[unit, Error]:\n    text = \"\\\"Nagi\\\"\"\n    decoded = try json_decode[view[str]](text)\n    {action}\n    return ok(print(decoded))\n");
        let error = checked(&source, true).unwrap_err();
        assert!(
            error.starts_with("line 6:") && error.contains("参照"),
            "{error}"
        );
    }
    let low = "fn main() -> Result[unit, Error] {\n    let text: str = \"\\\"Nagi\\\"\";\n    let decoded: view[str] = try json_decode[view[str]](text);\n    text = \"changed\";\n    return ok(print(decoded));\n}\n";
    let error = checked(low, false).unwrap_err();
    assert!(
        error.starts_with("line 4:") && error.contains("参照"),
        "{error}"
    );
}

#[test]
fn borrowed_json_origins_survive_aliases_match_arms_and_branch_joins() {
    for body in [
        "    borrowed = view(text)\n    decoded = try json_decode[view[str]](borrowed)\n    alias = decoded\n    borrowed = view(other)\n    take(text)\n    print(alias)\n",
        "    result = json_decode[view[str]](text)\n    match result:\n        case Ok(decoded):\n            take(text)\n            print(decoded)\n        case Err(_):\n            print(0)\n",
        "    decoded = [try json_decode[view[str]](text)][0]\n    take(text)\n    print(decoded)\n",
        "    decoded = try json_decode[view[str]](\"\\\"static\\\"\")\n    if True:\n        decoded = try json_decode[view[str]](text)\n    else:\n        print(0)\n    take(text)\n    print(decoded)\n",
    ] {
        let source = format!("def take(value: str):\n    print(value)\ndef main() -> Result[unit, Error]:\n    text = \"\\\"Nagi\\\"\"\n    other = \"\\\"other\\\"\"\n{body}    return ok(print(\"done\"))\n");
        assert!(checked(&source, true).unwrap_err().contains("参照"));
    }
}

#[test]
fn json_borrows_only_the_input_field_and_ends_with_the_match() {
    accepts("class Input:\n    text: str\n    tag: str\ndef main() -> Result[unit, Error]:\n    input = Input(text=\"\\\"Nagi\\\"\", tag=\"metadata\")\n    decoded = try json_decode[view[str]](input.text)\n    tag = input.tag\n    print(tag)\n    return ok(print(decoded))\n");
    accepts("def main() -> Result[unit, Error]:\n    text = \"\\\"Nagi\\\"\"\n    result = json_decode[view[str]](text)\n    match result:\n        case Ok(decoded):\n            alias = decoded\n            print(alias)\n        case Err(_):\n            print(0)\n    text = \"changed\"\n    return ok(print(text))\n");
}

#[test]
fn json_literal_input_and_immediate_temporary_input_remain_legal() {
    accepts("def make() -> str:\n    return \"\\\"Nagi\\\"\"\ndef main() -> Result[unit, Error]:\n    decoded = try json_decode[view[str]](\"\\\"static\\\"\")\n    print(decoded)\n    owned = copy(try json_decode[view[str]](make()))\n    print(owned)\n    match json_decode[view[str]](make()):\n        case Ok(part):\n            alias = part\n            print(alias)\n        case Err(_):\n            print(0)\n    return ok(print(\"done\"))\n");
    let source = "def make() -> str:\n    return \"\\\"Nagi\\\"\"\ndef main() -> Result[unit, Error]:\n    decoded = try json_decode[view[str]](make())\n    return ok(print(decoded))\n";
    assert!(checked(source, true)
        .unwrap_err()
        .contains("一時的な所有値"));
}

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn temporary_views_fail_in_check_and_build_before_cargo() {
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "nagi-view-origins-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    )));
    fs::create_dir(&fixture.0).unwrap();
    for (file, source, line, message) in [
        ("main.nagi", "def main():\n    borrowed = view(\"Nagi\")\n    print(borrowed)\n", 2, "一時的な所有値"),
        ("main.low", "fn main() -> unit {\n    let borrowed: view[str] = view(\"Nagi\");\n    print(borrowed);\n}\n", 2, "一時的な所有値"),
        ("main.nagi", "def main() -> Result[unit, Error]:\n    text = \"\\\"Nagi\\\"\"\n    decoded = try json_decode[view[str]](text)\n    text = \"changed\"\n    return ok(print(decoded))\n", 4, "参照中"),
    ] {
        fs::write(fixture.0.join(file), source).unwrap();
        for command in ["check", "build"] {
            let result = Command::new(env!("CARGO_BIN_EXE_nagic"))
                .args([command, file, "--no-project"])
                .current_dir(&fixture.0)
                .env("PATH", "")
                .env("NAGI_ROOT", fixture.0.join("missing-runtime"))
                .output().unwrap();
            let stderr = String::from_utf8_lossy(&result.stderr);
            assert!(!result.status.success() && stderr.contains(message) && stderr.contains(&format!("{file}:{line}")), "{stderr}");
            assert!(!stderr.contains("Cargo") && !stderr.contains("Rust backend"), "{stderr}");
            assert!(!fixture.0.join("build/main/src/main.rs").exists());
        }
    }
}
