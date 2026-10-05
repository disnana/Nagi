#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, parser};
use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn checked(source: &str, high: bool) -> Result<nagic::ast::Program, String> {
    let mut program = parser::parse(source, high)?;
    check::check(&mut program)?;
    Ok(program)
}

fn accepts_native(source: &str) {
    let high = checked(source, true).unwrap_or_else(|error| panic!("{source}\n{error}"));
    let low_source = emit::low(&high);
    let low = checked(&low_source, false).unwrap_or_else(|error| panic!("{low_source}\n{error}"));
    let folder = std::env::temp_dir().join(format!(
        "nagi-borrow-free-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&folder).unwrap();
    for (name, program) in [("high", &high), ("low", &low)] {
        let rust = folder.join(format!("{name}.rs"));
        fs::write(&rust, emit::rust(&checked_emission::seal(program)).unwrap()).unwrap();
        let result = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2021", "--crate-type=lib", "--emit=metadata"])
            .arg(&rust)
            .arg("-o")
            .arg(folder.join(format!("{name}.rmeta")))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{source}\n{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    fs::remove_dir_all(folder).unwrap();
}

fn rejects_both(source: &str) {
    let mut parsed = parser::parse(source, true).unwrap();
    // The failing return is checked after local types have been inferred.
    // Preserve those types when independently spelling the rejected Low.
    check::check(&mut parsed).expect_err(source);
    let low = emit::low(&parsed);
    for (source, high) in [(source, true), (low.as_str(), false)] {
        let error = checked(source, high).expect_err(source);
        assert!(
            error.contains("view escapes")
                || error.contains("scopeの外へ保存")
                || error.contains("externの戻り値にview"),
            "{source}\n{error}"
        );
    }
}

#[test]
fn absent_views_in_nullable_empty_lists_and_failures_can_be_returned() {
    for source in [
        "def absent() -> view[str]?:\n    return None\n",
        "def absent() -> List[view[str]]:\n    return []\n",
        "enum Problem:\n    Missing\ndef absent() -> Result[view[str], Problem]:\n    return fail(Problem.Missing)\n",
        "def absent() -> List[view[str]?]:\n    return [None]\n",
        "def absent() -> List[List[view[str]]]:\n    return [[], []]\n",
        "enum Problem:\n    Missing\ndef absent() -> Result[view[str]?, Problem]:\n    return ok(None)\n",
        "def absent() -> Option[Option[view[str]]]:\n    return some(None)\n",
        "enum Problem:\n    Missing\ndef absent() -> List[Result[view[str], Problem]]:\n    return [fail(Problem.Missing)]\n",
        "def absent() -> Result[view[str], str]:\n    problem = \"missing\"\n    return fail(problem)\n",
    ] {
        accepts_native(source);
    }
}

#[test]
fn direct_absent_returns_remain_legal_after_local_borrows() {
    for source in [
        "def absent(flag: bool) -> view[str]?:\n    value: view[str]? = None\n    if flag:\n        local = \"owned\"\n        value = some(view(local))\n        return None\n    return None\n",
        "def absent() -> List[view[str]?]:\n    values: List[view[str]?] = []\n    local = \"owned\"\n    for number in range(2):\n        append(values, some(view(local)))\n    return []\n",
        "enum Problem:\n    Missing\ndef absent() -> Result[view[str], Problem]:\n    local = \"owned\"\n    value: Result[view[str], Problem] = ok(view(local))\n    match value:\n        case Ok(part):\n            print(part)\n            return fail(Problem.Missing)\n        case Err(problem):\n            return fail(problem)\n",
    ] {
        accepts_native(source);
    }
}

#[test]
fn adding_local_views_to_absent_values_still_rejects_escape() {
    for source in [
        "def bad() -> view[str]?:\n    value: view[str]? = None\n    local = \"owned\"\n    value = some(view(local))\n    return value\n",
        "def bad() -> List[view[str]]:\n    value: List[view[str]] = []\n    local = \"owned\"\n    append(value, view(local))\n    return value\n",
        "enum Problem:\n    Missing\ndef bad() -> Result[view[str], Problem]:\n    value: Result[view[str], Problem] = fail(Problem.Missing)\n    local = \"owned\"\n    value = ok(view(local))\n    return value\n",
        "def bad(flag: bool) -> view[str]?:\n    value: view[str]? = None\n    local = \"owned\"\n    if flag:\n        value = some(view(local))\n    return value\n",
        "def bad() -> view[str]?:\n    value: view[str]? = None\n    local = \"owned\"\n    for number in range(2):\n        value = some(view(local))\n    return value\n",
        "def bad() -> List[view[str]?]:\n    value: List[view[str]?] = [None]\n    local = \"owned\"\n    append(value, some(view(local)))\n    return value\n",
    ] {
        rejects_both(source);
    }
}

#[test]
fn unknown_native_origins_do_not_gain_an_absence_proof() {
    for body in [
        "    return opaque()\n",
        "    value = opaque()\n    return value\n",
        "    value: view[str]? = None\n    if flag:\n        value = opaque()\n    return value\n",
        "    value: view[str]? = None\n    for number in range(2):\n        value = opaque()\n    return value\n",
    ] {
        rejects_both(&format!(
            "@rust(\"native::opaque\")\nextern def opaque() -> view[str]?\ndef bad(flag: bool) -> view[str]?:\n{body}"
        ));
    }
    // An ordinary function's view signature also does not prove that a
    // particular call returns None, even when its argument has no origins.
    for body in [
        "    return opaque(None)\n",
        "    value = opaque(None)\n    return value\n",
        "    value: view[str]? = None\n    if flag:\n        value = opaque(None)\n    return value\n",
        "    value: view[str]? = None\n    for number in range(2):\n        value = opaque(None)\n    return value\n",
    ] {
        rejects_both(&format!(
            "def opaque(value: view[str]?) -> view[str]?:\n    return value\ndef bad(flag: bool) -> view[str]?:\n{body}"
        ));
    }
}

#[test]
fn clearing_values_does_not_erase_rust_lifetime_constraints() {
    for source in [
        "def answer() -> view[str]?:\n    local = \"owned\"\n    value = some(view(local))\n    value = None\n    return value\n",
        "def answer(flag: bool) -> view[str]?:\n    local = \"owned\"\n    value = some(view(local))\n    if flag:\n        value = None\n    else:\n        value = None\n    return value\n",
        "def answer() -> view[str]?:\n    value: view[str]? = None\n    local = \"owned\"\n    value = some(view(local))\n    value = None\n    return value\n",
        "def answer(flag: bool) -> view[str]?:\n    value: view[str]? = None\n    if flag:\n        local = \"owned\"\n        value = some(view(local))\n        return None\n    return value\n",
        "def answer() -> List[view[str]]:\n    local = \"owned\"\n    value = [view(local)]\n    value = []\n    return value\n",
        "def answer() -> view[str]?:\n    local = \"owned\"\n    value = some(view(local))\n    value = None\n    alias = value\n    return alias\n",
        "enum Problem:\n    Missing\ndef answer() -> Result[view[str], Problem]:\n    local = \"owned\"\n    value: Result[view[str], Problem] = ok(view(local))\n    value = fail(Problem.Missing)\n    return value\n",
        "def answer() -> List[view[str]]:\n    local = \"owned\"\n    value = [view(local)]\n    value = []\n    return copy(view(value))\n",
        "def answer() -> view[str]?:\n    local = \"owned\"\n    value = some(view(local))\n    value = None\n    match value:\n        case Some(part):\n            return some(part)\n        case None:\n            return None\n",
        "def answer() -> view[str]?:\n    value: view[str]? = None\n    alias = value\n    local = \"owned\"\n    value = some(view(local))\n    value = None\n    return alias\n",
        "def answer() -> List[view[str]]:\n    values: List[view[str]] = []\n    alias = copy(view(values))\n    local = \"owned\"\n    append(values, view(local))\n    values = []\n    return alias\n",
        "def answer(flag: bool) -> view[str]?:\n    value: view[str]? = None\n    if flag:\n        return value\n    local = \"owned\"\n    value = some(view(local))\n    return None\n",
    ] {
        rejects_both(source);
    }
}
