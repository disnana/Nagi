use nagic::{check, emit, parser};

fn checked(source: &str) -> Result<nagic::ast::Program, String> {
    let mut program = parser::parse(source, true)?;
    check::check(&mut program)?;
    Ok(program)
}

#[test]
fn inferred_list_first_elements_can_move_owned_inputs_once() {
    for source in [
        "def identity(value: str) -> str:\n    return value\ndef main():\n    text = \"Nagi\"\n    values = [identity(text)]\n    print(len(values))\n",
        "class User:\n    name: str\ndef main():\n    text = \"Nagi\"\n    values = [User(name=text)]\n    print(len(values))\n",
        "def main():\n    text = \"Nagi\"\n    values = [some(text)]\n    print(len(values))\n",
        "def identity(value: str) -> str:\n    return value\ndef main():\n    text = \"Nagi\"\n    values = [[identity(text)]]\n    print(len(values))\n",
        "def main() -> Result[unit, Error]:\n    outcome = ok(\"Nagi\")\n    values = [try outcome]\n    return ok(print(len(values)))\n",
        "async def open(path: str) -> Result[str, Error]:\n    return ok(path)\nasync def start() -> Result[unit, Error]:\n    path = \":memory:\"\n    results = [await open(path)]\n    return ok(print(len(results)))\n",
    ] {
        let high = checked(source).unwrap_or_else(|error| panic!("{source}\n{error}"));
        let mut low = parser::parse(&emit::low(&high), false).unwrap();
        check::check(&mut low).unwrap();
    }
}

#[test]
fn inferred_lists_still_reject_second_moves_and_mixed_element_types() {
    for source in [
        "def identity(value: str) -> str:\n    return value\ndef main():\n    text = \"Nagi\"\n    values = [identity(text), identity(text)]\n",
        "def main():\n    text = \"Nagi\"\n    values = [some(text), some(text)]\n",
    ] {
        assert!(checked(source).unwrap_err().contains("text はmove後"));
    }
    assert!(checked("def main():\n    values = [1, \"Nagi\"]\n")
        .unwrap_err()
        .contains("expected i64"));
}

#[test]
fn explicit_list_element_types_still_contextualize_the_first_literal() {
    let high = checked("def main():\n    values: List[i8] = [1, 127]\n    other: List[i8] = []\n    append(other, values[0])\n").unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    assert!(checked("def main():\n    values: List[i8] = [128]\n")
        .unwrap_err()
        .contains("i8 の範囲"));
}

#[test]
fn record_list_iteration_accepts_copy_values_and_readonly_noncopy_borrows() {
    let high = checked("class Item:\n    amount: i64\n    quantity: i64\ndef main():\n    items = [Item(amount=20, quantity=2)]\n    for item in items:\n        print(item.amount * item.quantity)\n").unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();

    for (high, source) in [
        (true, "class Item:\n    amount: i64\n    label: str\ndef main():\n    items = [Item(amount=20, label=\"Nagi\")]\n    for item in items:\n        print(item.amount)\n"),
        (false, "record Item { amount: i64; label: str; }\nfn main() {\n    let items = [Item(amount=20, label=\"Nagi\")];\n    for item in items { print(item.amount); }\n}\n"),
    ] {
        let mut program = parser::parse(source, high).unwrap();
        check::check(&mut program).unwrap();
    }
}
