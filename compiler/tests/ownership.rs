use nagic::{check, emit, parser, source};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const MODEL: &str =
    "from std.ownership import move\nclass Person:\n    name: str\n    note: str\n    age: i64\n\n\
    class Group:\n    person: Person\n    title: str\n\n\
    def take(text: str):\n    print(text)\n\n\
    def take_person(person: Person):\n    print(person.age)\n\n";

fn program(body: &str) -> String {
    format!("{MODEL}def main():\n{body}")
}

fn checked(source: &str) -> Result<nagic::ast::Program, String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Input(PathBuf);
    impl Drop for Input {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let input = loop {
        let path = std::env::temp_dir().join(format!(
            "nagi-field-ownership-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&path) {
            Ok(()) => break Input(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("exclusive ownership fixture: {error}"),
        }
    };
    let path = input.0.join("main.nagi");
    fs::write(&path, source).unwrap();
    let mut p = source::load(&path, true)?.program;
    check::check(&mut p)?;
    Ok(p)
}

fn rejects(body: &str, place: &str) {
    let message = checked(&program(body)).unwrap_err();
    assert!(message.contains("move後"), "{message}");
    assert!(message.contains(place), "{message}");
}

fn accepts(body: &str) {
    let high = checked(&program(body)).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
}

#[test]
fn repeated_field_move_is_rejected_at_the_second_assignment() {
    let source = "class Person:\n    name: str\n\ndef main():\n    person = Person(name=\"凪\")\n    first = person.name\n    second = person.name\n    print(first)\n    print(second)\n";
    let message = checked(source).unwrap_err();
    assert!(message.starts_with("line 7:"), "{message}");
    assert!(message.contains("person.name はmove後"), "{message}");
    assert!(message.contains("copy(view(...))"), "{message}");
}

#[test]
fn fields_are_consumed_in_calls_records_lists_and_owned_wrappers() {
    for use_field in [
        "first = person.name",
        "take(person.name)",
        "other = Person(name=person.name, note=\"other\", age=1)",
        "names = [person.name]",
        "wrapped = ok(person.name)",
        "wrapped = some(person.name)",
        "shared_name = share(person.name)",
        "page = html(person.name)",
    ] {
        rejects(
            &format!("    person = Person(name=\"Nagi\", note=\"note\", age=1)\n    {use_field}\n    print(person.name)\n"),
            "person.name",
        );
    }
    rejects(
        "    person = Person(name=\"Nagi\", note=\"note\", age=1)\n    names = [person.name, person.name]\n",
        "person.name",
    );
}

#[test]
fn a_partially_moved_record_cannot_be_used_as_a_whole() {
    for use_person in [
        "other = move(person)",
        "take_person(person)",
        "group = Group(person=person, title=\"group\")",
    ] {
        rejects(
            &format!("    person = Person(name=\"Nagi\", note=\"note\", age=1)\n    name = person.name\n    {use_person}\n"),
            "person.name",
        );
    }
}

#[test]
fn nested_moves_block_the_moved_path_and_its_ancestors_and_children() {
    for use_group in [
        "print(group.person.name)",
        "other = group.person",
        "other = move(group)",
    ] {
        rejects(
            &format!("    group = Group(person=Person(name=\"Nagi\", note=\"note\", age=1), title=\"group\")\n    name = group.person.name\n    {use_group}\n"),
            "group.person.name",
        );
    }
    rejects(
        "    group = Group(person=Person(name=\"Nagi\", note=\"note\", age=1), title=\"group\")\n    person = group.person\n    print(group.person.age)\n",
        "group.person",
    );
}

#[test]
fn unrelated_owned_fields_and_copy_fields_remain_available() {
    accepts("    person = Person(name=\"Nagi\", note=\"note\", age=1)\n    name = person.name\n    note = person.note\n    first_age = person.age\n    second_age = person.age\n    print(name)\n    print(note)\n    print(first_age + second_age)\n");
    accepts("    group = Group(person=Person(name=\"Nagi\", note=\"note\", age=1), title=\"group\")\n    name = group.person.name\n    title = group.title\n    note = group.person.note\n    print(group.person.age)\n    print(name)\n    print(title)\n    print(note)\n");
}

#[test]
fn field_names_are_compared_as_path_segments() {
    let source = "class Names:\n    name: str\n    name_suffix: str\ndef main():\n    names = Names(name=\"Nagi\", name_suffix=\"suffix\")\n    first = names.name\n    second = names.name_suffix\n    print(first)\n    print(second)\n";
    checked(source).unwrap();
}

#[test]
fn reading_borrowing_and_copying_do_not_move_a_field() {
    accepts("    person = Person(name=\"Nagi\", note=\"note\", age=1)\n    print(person.name)\n    print(person.name)\n    print(len(view(person.name)))\n    duplicate = copy(view(person.name))\n    original = person.name\n    print(duplicate)\n    print(original)\n    print(person.age)\n");
}

#[test]
fn a_borrowed_field_cannot_be_moved_but_copy_fields_can_be_read() {
    let source = program("    person = Person(name=\"Nagi\", note=\"note\", age=1)\n    borrowed = view(person.name)\n    name = person.name\n    print(copy(borrowed))\n");
    let message = checked(&source).unwrap_err();
    assert!(message.contains("参照"), "{message}");
    accepts("    person = Person(name=\"Nagi\", note=\"note\", age=1)\n    borrowed = view(person.name)\n    age = person.age\n    print(age)\n    print(copy(borrowed))\n");
}

#[test]
fn field_moves_are_merged_from_branches_match_arms_and_loops() {
    for body in [
        "    if True:\n        name = person.name\n    else:\n        print(0)\n",
        "    if True:\n        print(0)\n    else:\n        name = person.name\n",
        "    match parse_i64(\"1\"):\n        case Ok(_):\n            name = person.name\n        case Err(_):\n            print(0)\n",
        "    for number in range(1):\n        name = person.name\n",
        "    while False:\n        name = person.name\n",
    ] {
        rejects(
            &format!("    person = Person(name=\"Nagi\", note=\"note\", age=1)\n{body}    print(person.name)\n"),
            "person.name",
        );
    }
    accepts("    person = Person(name=\"Nagi\", note=\"note\", age=1)\n    if True:\n        name = person.name\n    else:\n        note = person.note\n    print(person.age)\n");
}

#[test]
fn field_moves_are_merged_from_async_scopes() {
    let source = format!("{MODEL}async def main() -> Result[unit, Error]:\n    person = Person(name=\"Nagi\", note=\"note\", age=1)\n    async with scope:\n        name = person.name\n    print(person.name)\n    return ok(print(0))\n");
    let message = checked(&source).unwrap_err();
    assert!(message.contains("person.name はmove後"), "{message}");
}

#[test]
fn assigning_a_new_record_restores_its_fields() {
    accepts("    person = Person(name=\"old\", note=\"note\", age=1)\n    old_name = person.name\n    person = Person(name=\"new\", note=\"new note\", age=2)\n    take_person(person)\n    print(old_name)\n");
}

#[test]
fn copy_record_fields_can_be_used_repeatedly() {
    let source = "class Point:\n    x: i64\nclass Pair:\n    point: Point\ndef main():\n    pair = Pair(point=Point(x=3))\n    first = pair.point\n    second = pair.point\n    print(first.x + second.x + pair.point.x)\n";
    checked(source).unwrap();
}

#[test]
fn field_access_after_moving_the_whole_record_is_still_rejected() {
    rejects("    person = Person(name=\"Nagi\", note=\"note\", age=1)\n    take_person(person)\n    print(person.age)\n", "person");
}

#[test]
fn standalone_low_checks_field_moves() {
    let source = "record Person { name: str; age: i64; }\nfn main() -> unit {\n    let person: Person = Person(name=\"Nagi\", age=1);\n    let first: str = person.name;\n    let second: str = person.name;\n}\n";
    let mut low = parser::parse(source, false).unwrap();
    let message = check::check(&mut low).unwrap_err();
    assert!(message.starts_with("line 5:"), "{message}");
    assert!(message.contains("person.name"), "{message}");
}
