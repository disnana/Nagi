use nagic::{check, emit, parser, source};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi-ownership-boundaries-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn checked(text: &str, high: bool) -> Result<nagic::ast::Program, String> {
    let mut program = parser::parse(text, high)?;
    check::check(&mut program)?;
    Ok(program)
}

fn rejects_both(text: &str, reason: &str) {
    let parsed = parser::parse(text, true).unwrap();
    // Parse the Low spelling independently, without copying typed expressions
    // or ownership state from the High checker.
    let low = emit::low(&parsed);
    for (text, high) in [(text, true), (low.as_str(), false)] {
        let error = checked(text, high).expect_err(text);
        assert!(error.contains(reason), "{text}\n{error}");
    }
}

fn accepts_native(text: &str) {
    let high = checked(text, true).unwrap_or_else(|error| panic!("{text}\n{error}"));
    let low_text = emit::low(&high);
    let low = checked(&low_text, false).unwrap_or_else(|error| panic!("{low_text}\n{error}"));
    let rust = emit::rust(&low).unwrap();
    assert_eq!(rust, emit::rust(&high).unwrap());
    let fixture = Fixture::new();
    let source = fixture.0.join("main.rs");
    fs::write(&source, rust).unwrap();
    let result = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2021", "--emit=metadata"])
        .arg(&source)
        .arg("-o")
        .arg(fixture.0.join("main.rmeta"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{text}\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn index_evaluation_holds_the_container_but_releases_it_afterwards() {
    for left in ["values", "view(values)"] {
        rejects_both(&format!("def take(values: List[i64]) -> i64:\n    return 0\ndef main():\n    values = [42]\n    print({left}[take(values)])\n"), "同じ式で先に参照");
    }
    let low = "fn take(values: List[i64]) -> i64 { return 0; }\nfn main() -> unit {\n    let values: List[i64] = [42];\n    print(values[take(values)]);\n}\n";
    let error = checked(low, false).unwrap_err();
    assert!(
        error.starts_with("line 4:") && error.contains("同じ式で先に参照"),
        "{error}"
    );
    accepts_native("def take(values: List[i64]) -> i64:\n    return 0\ndef locate(values: view[i64]) -> i64:\n    return len(values) - 1\ndef use_views(values: List[view[str]]):\n    print(len(values))\ndef main():\n    values = [42]\n    other = [99]\n    print(values[take(other)])\n    print(values[locate(view(values))])\n    print(values[take(copy(view(values)))])\n    print(take(values))\n    text = \"Nagi\"\n    parts = [view(text)]\n    picked = parts[0]\n    use_views(parts)\n    print(picked)\n");
}

#[test]
fn branch_loop_and_payload_owners_cannot_escape_in_outer_views() {
    for body in [
        "    if True:\n        local = \"inner\"\n        alias = view(local)\n",
        "    for number in range(1):\n        local = \"inner\"\n        alias = view(local)\n",
        "    while False:\n        local = \"inner\"\n        alias = view(local)\n",
        "    value: Option[str] = some(\"inner\")\n    match value:\n        case Some(local):\n            alias = view(local)\n        case None:\n            print(0)\n",
        "    value: Result[str, i64] = ok(\"inner\")\n    match value:\n        case Ok(local):\n            alias = view(local)\n        case Err(_):\n            print(0)\n",
    ] {
        rejects_both(&format!("def main():\n    outer = \"outer\"\n    alias = view(outer)\n{body}    print(alias)\n"), "scopeの外へ保存");
    }
    rejects_both("enum Message:\n    Text(value: str)\n    Empty\ndef main():\n    outer = \"outer\"\n    alias = view(outer)\n    message = Message.Text(\"inner\")\n    match message:\n        case Message.Text(local):\n            alias = view(local)\n        case Message.Empty:\n            print(0)\n    print(alias)\n", "scopeの外へ保存");
    let low = "fn main() -> unit {\n    let outer: str = \"outer\";\n    let alias: view[str] = view(outer);\n    if True {\n        let local: str = \"inner\";\n        alias = view(local);\n    }\n    print(alias);\n}\n";
    let error = checked(low, false).unwrap_err();
    assert!(
        error.starts_with("line 4:") && error.contains("scopeの外へ保存"),
        "{error}"
    );
}

#[test]
fn nested_view_containers_cannot_keep_child_scope_owners() {
    rejects_both("def main():\n    outer = \"outer\"\n    alias: Option[view[str]] = some(view(outer))\n    if True:\n        local = \"inner\"\n        alias = some(view(local))\n    match alias:\n        case Some(part):\n            print(part)\n        case None:\n            print(0)\n", "scopeの外へ保存");
    rejects_both("def main():\n    outer = \"outer\"\n    alias: Option[view[str]] = some(view(outer))\n    value = some(\"inner\")\n    match value:\n        case Some(local):\n            alias = some(view(local))\n        case None:\n            print(0)\n    match alias:\n        case Some(part):\n            print(part)\n        case None:\n            print(0)\n", "scopeの外へ保存");
    rejects_both("def main():\n    outer = \"outer\"\n    parts = [view(outer)]\n    if True:\n        local = \"inner\"\n        append(parts, view(local))\n    print(parts[0])\n", "scopeの外へ保存");
}

#[test]
fn temporary_iterator_views_cannot_escape_but_body_use_and_static_views_remain_valid() {
    for iterator in ["[view(make())]", "[view(outer), view(make())]"] {
        rejects_both(&format!("def make() -> str:\n    return \"inner\"\ndef main():\n    outer = \"outer\"\n    alias = view(outer)\n    for part in {iterator}:\n        alias = part\n    print(alias)\n"), "scopeの外へ保存");
    }
    rejects_both("def make() -> str:\n    return \"inner\"\ndef main():\n    outer = \"outer\"\n    parts = [view(outer)]\n    for part in [view(make())]:\n        append(parts, part)\n    print(parts[0])\n", "scopeの外へ保存");
    rejects_both("def make() -> str:\n    return \"inner\"\ndef escape(fallback: view[str]) -> view[str]:\n    for part in [view(make())]:\n        return part\n    return fallback\n", "view escapes");
    accepts_native("def make() -> str:\n    return \"inner\"\ndef static_part(fallback: view[str]) -> view[str]:\n    for part in [view(\"literal\")]:\n        return part\n    return fallback\ndef main():\n    for part in [view(make())]:\n        local_alias = part\n        print(local_alias)\n    copied = \"outer\"\n    for part in [view(make())]:\n        copied = copy(part)\n    print(copied)\n    outer = \"outer\"\n    inside = \"inner\"\n    alias = view(outer)\n    for part in [view(inside), view(\"literal\")]:\n        alias = part\n    print(alias)\n    for part in [view(\"literal\")]:\n        alias = part\n    print(alias)\n    for alias in [view(make())]:\n        print(alias)\n    print(alias)\n    print(static_part(view(outer)))\n");
}

#[test]
fn copied_iterator_elements_keep_their_own_borrow_depth() {
    accepts_native("def parts(text: view[str]) -> List[view[str]]:\n    return [text]\ndef main():\n    owner = \"outer\"\n    alias = view(owner)\n    for inner in [view(parts(view(owner)))]:\n        alias = inner[0]\n    print(alias)\n    for inner in [view([view(owner)])]:\n        alias = inner[0]\n    print(alias)\n    for inner in [view([view(\"literal\")])]:\n        alias = inner[0]\n    print(alias)\n");
    rejects_both("def parts(text: view[str]) -> List[view[str]]:\n    return [text]\ndef main():\n    owner = \"outer\"\n    stored = [view(owner)]\n    alias = view(stored)\n    for inner in [view(parts(view(owner)))]:\n        alias = inner\n    print(alias[0])\n", "scopeの外へ保存");
    rejects_both("def make() -> str:\n    return \"inner\"\ndef main():\n    owner = \"outer\"\n    alias = view(owner)\n    for inner in [view([view(make())])]:\n        alias = inner[0]\n    print(alias)\n", "scopeの外へ保存");
}

#[test]
fn outer_owners_local_copies_restored_aliases_and_return_paths_remain_valid() {
    accepts_native("def parameter(part: view[str]) -> view[str]:\n    alias = part\n    if True:\n        alias = part\n    return alias\ndef returned() -> str:\n    outer = \"outer\"\n    alias = view(outer)\n    if True:\n        local = \"inner\"\n        alias = view(local)\n        return copy(alias)\n    return copy(alias)\ndef main():\n    outer = \"outer\"\n    inner = \"inner\"\n    alias = view(outer)\n    if True:\n        alias = view(inner)\n    print(alias)\n    if True:\n        local = \"local\"\n        local_view = view(local)\n        print(local_view)\n        copied = copy(local_view)\n        alias = view(local)\n        print(alias)\n        alias = view(outer)\n        print(copied)\n    for number in range(2):\n        local = \"loop\"\n        alias = view(local)\n        print(alias)\n        alias = view(outer)\n    value: Option[view[str]] = some(view(outer))\n    match value:\n        case Some(part):\n            alias = part\n        case None:\n            print(0)\n    print(parameter(alias))\n    print(returned())\n");
    // Shadowing an owner's name does not shorten an already created view's
    // lifetime; its original binding still lives outside the loop.
    accepts_native("def parameter(values: List[view[str]]):\n    alias = values[0]\n    for values in range(1):\n        if True:\n            print(values)\n        print(alias)\ndef main():\n    owner = \"outer\"\n    alias = view(owner)\n    for owner in range(1):\n        if True:\n            print(owner)\n        print(alias)\n    parameter([alias])\n");
}

#[test]
fn static_resource_views_remain_valid_across_branch_scopes() {
    let fixture = Fixture::new();
    let path = fixture.0.join("main.nagi");
    fs::write(&path, "import std.http.server as http\ndef phrase() -> view[str]:\n    result = http.Status.OK.phrase\n    if True:\n        result = http.Status.NOT_FOUND.phrase\n    return result\n").unwrap();
    let mut loaded = source::load(&path, true).unwrap();
    check::check(&mut loaded.program).unwrap();
    checked(&emit::low(&loaded.program), false).unwrap();
}
