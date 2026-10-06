#[path = "support/checked_emission.rs"]
mod checked_emission;
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
            "nagi-view-container-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn compile_and_run(&self, name: &str, program: &nagic::ast::Program) {
        let source = self.0.join(format!("{name}.rs"));
        let binary = self
            .0
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        fs::write(
            &source,
            emit::rust(&checked_emission::seal(program)).unwrap() + ASSERTIONS,
        )
        .unwrap();
        let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2021", "--test"])
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(&binary).output().unwrap();
        assert!(
            output.status.success(),
            "{name}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn checked(text: &str, high: bool) -> nagic::ast::Program {
    let fixture = Fixture::new();
    let path = fixture
        .0
        .join(if high { "input.nagi" } else { "input.low" });
    fs::write(&path, text).unwrap();
    let mut program = source::load(&path, high)
        .unwrap_or_else(|error| panic!("{text}\n{error}"))
        .program;
    check::check(&mut program).unwrap_or_else(|error| panic!("{text}\n{error}"));
    program
}

#[test]
fn owning_view_container_rebinding_runs_in_high_saved_low_and_handwritten_low() {
    let high = checked(HIGH, true);
    // Reparse and check the serialized Low independently. This exercises the
    // same ownership contract without relying on High-only annotations.
    let saved_low = checked(&emit::low(&high), false);
    let handwritten_low = checked(LOW, false);
    let fixture = Fixture::new();

    for (name, program) in [
        ("high", &high),
        ("saved-low", &saved_low),
        ("handwritten-low", &handwritten_low),
    ] {
        fixture.compile_and_run(name, program);
    }
}

#[test]
fn local_container_escape_stays_rejected_in_high_and_low() {
    for (text, high) in [(BAD_ESCAPE_HIGH, true), (BAD_ESCAPE_LOW, false)] {
        let mut program =
            parser::parse(text, high).unwrap_or_else(|error| panic!("{text}\n{error}"));
        let error = check::check(&mut program).expect_err(text);
        assert!(error.contains("内側のscopeの所有値"), "{text}\n{error}");
    }
}

#[test]
fn move_phi_and_mutation_fixtures_use_the_container_flow_plan() {
    for (source, high) in [(HIGH, true), (LOW, false)] {
        let program = checked(source, high);
        let rust = emit::rust(&checked_emission::seal(&program)).unwrap();
        for name in [
            "nested_terminal",
            "continuing_phi",
            "self_move_then_reinit",
            "maybe_move_then_reinit",
            "short_circuit_consume",
            "short_circuit_join",
            "short_circuit_condition",
            "append_then_restore",
            "hygienic_restore",
            "multiple_candidate_move",
            "cross_candidate_move",
            "branch_append_restore",
            "rhs_append_restore",
            "assigned_append_restore",
            "condition_append_restore",
            "condition_append_consume",
        ] {
            let symbol = &program.modules.resolve_root_path(name).unwrap().symbol;
            let function = rust.split(&format!("pub fn {symbol}<")).nth(1).unwrap();
            let function = function.split("\n}").next().unwrap();
            assert!(
                function.contains("__nagi_view_flow_"),
                "{name} did not use flow lowering"
            );
        }
    }
}

#[test]
fn restore_rhs_keeps_its_source_line_and_generated_slots_are_synthetic() {
    let program = checked(HIGH, true);
    let generated = emit::rust_with_lines(&checked_emission::seal(&program)).unwrap();
    let restore = &program
        .modules
        .resolve_root_path("container_restore")
        .unwrap()
        .symbol;
    let mut in_restore = false;
    let mut synthetic_container_declarations = 0;
    let mut saw_original_saved = 0;
    let mut saw_original_restore = 0;

    for (index, line) in generated.text.lines().enumerate() {
        if line.contains(&format!("pub fn {restore}<")) {
            in_restore = true;
        }
        if !in_restore {
            continue;
        }
        if line.trim_start().starts_with("let ")
            && line.contains("::std::vec::Vec<&::std::primitive::str>")
            && !line.contains('=')
        {
            assert_eq!(generated.line_origin(index + 1), None);
            synthetic_container_declarations += 1;
        }
        if line.trim_start().starts_with("let mut saved:") {
            // canonical move import追加により元saved bindingはline 3。
            assert_eq!(generated.line_origin(index + 1), Some(3));
            saw_original_saved += 1;
        }
        // by-value identity生成先へ同期。元restore RHSの1回観測は維持。
        if line.trim_start().starts_with("let ")
            && line
                .trim_end()
                .ends_with("= ::std::convert::identity(saved);")
        {
            // restore RHSの元位置も同じimport分だけ+1。
            assert_eq!(generated.line_origin(index + 1), Some(7));
            saw_original_restore += 1;
        }
        if line == "}" {
            break;
        }
    }

    assert!(
        synthetic_container_declarations > 0,
        "expected source-less typed container declarations"
    );
    assert_eq!(saw_original_restore, 1, "expected one mapped restore RHS");
    assert_eq!(saw_original_saved, 1, "expected one mapped saved binding");
}

const HIGH: &str = r#"from std.ownership import move
def container_restore(flag: bool, parts: List[view[str]]) -> List[view[str]]:
    saved = move(parts)
    local = "temporary"
    if flag:
        parts = [view(local)]
    parts = move(saved)
    return parts

def terminal_restore(flag: bool, parts: List[view[str]]) -> List[view[str]]:
    alias = [parts[0]]
    if flag:
        local = "terminal temporary"
        alias = [view(local)]
        alias = [parts[0]]
        return alias
    return alias

def straightline_restore(parts: List[view[str]]) -> List[view[str]]:
    saved = move(parts)
    local = "straightline temporary"
    parts = [view(local)]
    parts = move(saved)
    return parts

def terminal_update(flag: bool, parts: List[view[str]], replacement: List[view[str]]) -> List[view[str]]:
    if flag:
        parts = move(replacement)
        return parts
    return parts

def nested_terminal(outer: bool, inner: bool, parts: List[view[str]], first: List[view[str]], second: List[view[str]]) -> List[view[str]]:
    saved = move(parts)
    local = "nested temporary"
    parts = [view(local)]
    parts = move(saved)
    if outer:
        if inner:
            parts = move(first)
            return parts
        parts = move(second)
        return parts
    return parts

def continuing_phi(flag: bool, original: List[view[str]], replacement: List[view[str]], fallback: List[view[str]]) -> List[view[str]]:
    local = "phi temporary"
    selected = [view(local)]
    selected = move(replacement)
    if flag:
        selected = move(original)
    else:
        selected = move(fallback)
    return selected

def self_move_then_reinit(flag: bool, parts: List[view[str]], replacement: List[view[str]]) -> List[view[str]]:
    saved = move(parts)
    local = "self move temporary"
    parts = [view(local)]
    parts = move(saved)
    parts = move(parts)
    if flag:
        parts = move(replacement)
    return parts

def maybe_move_then_reinit(flag: bool, parts: List[view[str]], replacement: List[view[str]]) -> List[view[str]]:
    saved = move(parts)
    local = "maybe move temporary"
    parts = [view(local)]
    parts = move(saved)
    if flag:
        moved = move(parts)
    parts = move(replacement)
    return parts

def consume_views(parts: List[view[str]]) -> bool:
    return len(parts) > 0

def short_circuit_consume(flag: bool, part: view[str]) -> List[view[str]]:
    local = "short circuit temporary"
    parts = [view(local)]
    selected = flag and consume_views(parts)
    parts = [part]
    return parts

def short_circuit_join(flag: bool, branch: bool, part: view[str]) -> List[view[str]]:
    local = "join temporary"
    parts = [view(local)]
    selected = flag or consume_views(parts)
    if branch:
        parts = [part]
    parts = [part]
    return parts

def short_circuit_condition(flag: bool, part: view[str]) -> List[view[str]]:
    local = "condition temporary"
    parts = [view(local)]
    if flag and consume_views(parts):
        selected = True
    parts = [part]
    return parts

def append_then_restore(part: view[str]) -> List[view[str]]:
    parts: List[view[str]] = []
    local = "appended temporary"
    append(parts, view(local))
    parts = [part]
    return parts

def hygienic_restore(part: view[str]) -> List[view[str]]:
    __nagi_view_flow_rhs_0 = [part]
    type = "hygiene temporary"
    __nagi_view_flow_rhs_0 = [view(type)]
    __nagi_view_flow_rhs_0 = [part]
    return __nagi_view_flow_rhs_0

def multiple_candidate_move(flag: bool, choose: bool, first_part: view[str], second_part: view[str]) -> List[view[str]]:
    local = "multiple candidates"
    first = [view(local)]
    first = [first_part]
    second = [view(local)]
    second = [second_part]
    if flag:
        first = move(second)
    else:
        second = [second_part]
    second = [second_part]
    if choose:
        return first
    return second

def cross_candidate_move(flag: bool, choose: bool, first_part: view[str], second_part: view[str]) -> List[view[str]]:
    local = "cross candidate moves"
    first = [view(local)]
    first = [first_part]
    second = [view(local)]
    second = [second_part]
    if flag:
        first = move(second)
    else:
        second = move(first)
    first = [first_part]
    second = [second_part]
    if choose:
        return first
    return second

def make_views(ignored: unit, part: view[str]) -> List[view[str]]:
    return [part]

def accepts_unit(ignored: unit) -> bool:
    return True

def consumes_after_unit(ignored: unit, parts: List[view[str]]) -> bool:
    return len(parts) > 0

def branch_append_restore(flag: bool, part: view[str]) -> List[view[str]]:
    parts = [part]
    if flag:
        local = "branch appended temporary"
        append(parts, view(local))
        parts = [part]
    return parts

def rhs_append_restore(flag: bool, part: view[str]) -> List[view[str]]:
    parts = [part]
    if flag:
        local = "RHS appended temporary"
        parts = make_views(append(parts, view(local)), part)
    return parts

def assigned_append_restore(flag: bool, part: view[str]) -> List[view[str]]:
    parts = [part]
    if flag:
        local = "assigned append temporary"
        ignored = append(parts, view(local))
        parts = [part]
    return parts

def condition_append_restore(flag: bool, part: view[str]) -> List[view[str]]:
    local = "condition append temporary"
    parts = [part]
    if flag and accepts_unit(append(parts, view(local))):
        selected = True
    parts = [part]
    return parts

def condition_append_consume(flag: bool, part: view[str]) -> List[view[str]]:
    local = "consumed condition append temporary"
    parts = [part]
    if flag and consumes_after_unit(append(parts, view(local)), parts):
        selected = True
    parts = [part]
    return parts

def ordinary_list(flag: bool, values: List[i64], replacement: List[i64]) -> List[i64]:
    if flag:
        values = move(replacement)
    return values

def ordinary_view(flag: bool, part: view[str], replacement: view[str]) -> view[str]:
    if flag:
        part = replacement
    return part
"#;

// This Low is written independently so the same flow is checked without
// relying on emit::low's serialization decisions.
const LOW: &str = r#"from std.ownership import move
fn container_restore(flag: bool, parts: List[view[str]]) -> List[view[str]] {
    let saved: List[view[str]] = move(parts);
    let local: str = "temporary";
    if flag {
        parts = [view(local)];
    }
    parts = move(saved);
    return parts;
}

fn terminal_restore(flag: bool, parts: List[view[str]]) -> List[view[str]] {
    let alias: List[view[str]] = [parts[0]];
    if flag {
        let local: str = "terminal temporary";
        alias = [view(local)];
        alias = [parts[0]];
        return alias;
    }
    return alias;
}

fn straightline_restore(parts: List[view[str]]) -> List[view[str]] {
    let saved: List[view[str]] = move(parts);
    let local: str = "straightline temporary";
    parts = [view(local)];
    parts = move(saved);
    return parts;
}

fn terminal_update(flag: bool, parts: List[view[str]], replacement: List[view[str]]) -> List[view[str]] {
    if flag {
        parts = move(replacement);
        return parts;
    }
    return parts;
}

fn nested_terminal(outer: bool, inner: bool, parts: List[view[str]], first: List[view[str]], second: List[view[str]]) -> List[view[str]] {
    let saved: List[view[str]] = move(parts);
    let local: str = "nested temporary";
    parts = [view(local)];
    parts = move(saved);
    if outer {
        if inner {
            parts = move(first);
            return parts;
        }
        parts = move(second);
        return parts;
    }
    return parts;
}

fn continuing_phi(flag: bool, original: List[view[str]], replacement: List[view[str]], fallback: List[view[str]]) -> List[view[str]] {
    let local: str = "phi temporary";
    let selected: List[view[str]] = [view(local)];
    selected = move(replacement);
    if flag {
        selected = move(original);
    } else {
        selected = move(fallback);
    }
    return selected;
}

fn self_move_then_reinit(flag: bool, parts: List[view[str]], replacement: List[view[str]]) -> List[view[str]] {
    let saved: List[view[str]] = move(parts);
    let local: str = "self move temporary";
    parts = [view(local)];
    parts = move(saved);
    parts = move(parts);
    if flag {
        parts = move(replacement);
    }
    return parts;
}

fn maybe_move_then_reinit(flag: bool, parts: List[view[str]], replacement: List[view[str]]) -> List[view[str]] {
    let saved: List[view[str]] = move(parts);
    let local: str = "maybe move temporary";
    parts = [view(local)];
    parts = move(saved);
    if flag {
        let moved: List[view[str]] = move(parts);
    }
    parts = move(replacement);
    return parts;
}

fn consume_views(parts: List[view[str]]) -> bool {
    return len(parts) > 0;
}

fn short_circuit_consume(flag: bool, part: view[str]) -> List[view[str]] {
    let local: str = "short circuit temporary";
    let parts: List[view[str]] = [view(local)];
    let selected: bool = flag and consume_views(parts);
    parts = [part];
    return parts;
}

fn short_circuit_join(flag: bool, branch: bool, part: view[str]) -> List[view[str]] {
    let local: str = "join temporary";
    let parts: List[view[str]] = [view(local)];
    let selected: bool = flag or consume_views(parts);
    if branch {
        parts = [part];
    }
    parts = [part];
    return parts;
}

fn short_circuit_condition(flag: bool, part: view[str]) -> List[view[str]] {
    let local: str = "condition temporary";
    let parts: List[view[str]] = [view(local)];
    if flag and consume_views(parts) {
        let selected: bool = true;
    }
    parts = [part];
    return parts;
}

fn append_then_restore(part: view[str]) -> List[view[str]] {
    let parts: List[view[str]] = [];
    let local: str = "appended temporary";
    append(parts, view(local));
    parts = [part];
    return parts;
}

fn hygienic_restore(part: view[str]) -> List[view[str]] {
    let __nagi_view_flow_rhs_0: List[view[str]] = [part];
    let type: str = "hygiene temporary";
    __nagi_view_flow_rhs_0 = [view(type)];
    __nagi_view_flow_rhs_0 = [part];
    return __nagi_view_flow_rhs_0;
}

fn multiple_candidate_move(flag: bool, choose: bool, first_part: view[str], second_part: view[str]) -> List[view[str]] {
    let local: str = "multiple candidates";
    let first: List[view[str]] = [view(local)];
    first = [first_part];
    let second: List[view[str]] = [view(local)];
    second = [second_part];
    if flag {
        first = move(second);
    } else {
        second = [second_part];
    }
    second = [second_part];
    if choose {
        return first;
    }
    return second;
}

fn cross_candidate_move(flag: bool, choose: bool, first_part: view[str], second_part: view[str]) -> List[view[str]] {
    let local: str = "cross candidate moves";
    let first: List[view[str]] = [view(local)];
    first = [first_part];
    let second: List[view[str]] = [view(local)];
    second = [second_part];
    if flag {
        first = move(second);
    } else {
        second = move(first);
    }
    first = [first_part];
    second = [second_part];
    if choose {
        return first;
    }
    return second;
}

fn make_views(ignored: unit, part: view[str]) -> List[view[str]] {
    return [part];
}

fn accepts_unit(ignored: unit) -> bool {
    return true;
}

fn consumes_after_unit(ignored: unit, parts: List[view[str]]) -> bool {
    return len(parts) > 0;
}

fn branch_append_restore(flag: bool, part: view[str]) -> List[view[str]] {
    let parts: List[view[str]] = [part];
    if flag {
        let local: str = "branch appended temporary";
        append(parts, view(local));
        parts = [part];
    }
    return parts;
}

fn rhs_append_restore(flag: bool, part: view[str]) -> List[view[str]] {
    let parts: List[view[str]] = [part];
    if flag {
        let local: str = "RHS appended temporary";
        parts = make_views(append(parts, view(local)), part);
    }
    return parts;
}

fn assigned_append_restore(flag: bool, part: view[str]) -> List[view[str]] {
    let parts: List[view[str]] = [part];
    if flag {
        let local: str = "assigned append temporary";
        let ignored: unit = append(parts, view(local));
        parts = [part];
    }
    return parts;
}

fn condition_append_restore(flag: bool, part: view[str]) -> List[view[str]] {
    let local: str = "condition append temporary";
    let parts: List[view[str]] = [part];
    if flag and accepts_unit(append(parts, view(local))) {
        let selected: bool = true;
    }
    parts = [part];
    return parts;
}

fn condition_append_consume(flag: bool, part: view[str]) -> List[view[str]] {
    let local: str = "consumed condition append temporary";
    let parts: List[view[str]] = [part];
    if flag and consumes_after_unit(append(parts, view(local)), parts) {
        let selected: bool = true;
    }
    parts = [part];
    return parts;
}

fn ordinary_list(flag: bool, values: List[i64], replacement: List[i64]) -> List[i64] {
    if flag {
        values = move(replacement);
    }
    return values;
}

fn ordinary_view(flag: bool, part: view[str], replacement: view[str]) -> view[str] {
    if flag {
        part = replacement;
    }
    return part;
}
"#;

const BAD_ESCAPE_HIGH: &str = r#"def bad(flag: bool, parts: List[view[str]]) -> List[view[str]]:
    if flag:
        local = "branch-local"
        parts = [view(local)]
    return parts
"#;

const BAD_ESCAPE_LOW: &str = r#"fn bad(flag: bool, parts: List[view[str]]) -> List[view[str]] {
    if flag {
        let local: str = "branch-local";
        parts = [view(local)];
    }
    return parts;
}
"#;

const ASSERTIONS: &str = r#"
#[test]
fn container_restoration_returns_the_callers_values() {
    let keep = String::from("keep");
    assert_eq!(container_restore(false, vec![keep.as_str()]), vec!["keep"]);
    assert_eq!(container_restore(true, vec![keep.as_str()]), vec!["keep"]);
    let extra = String::from("extra");
    let terminal_parts = vec![keep.as_str(), extra.as_str()];
    assert_eq!(terminal_restore(false, terminal_parts.clone()), vec!["keep"]);
    assert_eq!(terminal_restore(true, terminal_parts), vec!["keep"]);

    let direct = String::from("direct");
    assert_eq!(straightline_restore(vec![direct.as_str()]), vec!["direct"]);
}

#[test]
fn terminal_and_nested_updates_return_the_selected_container() {
    let original = String::from("original");
    let first = String::from("first");
    let second = String::from("second");
    assert_eq!(terminal_update(false, vec![original.as_str()], vec![first.as_str()]), vec!["original"]);
    assert_eq!(terminal_update(true, vec![original.as_str()], vec![first.as_str()]), vec!["first"]);

    assert_eq!(nested_terminal(false, false, vec![original.as_str()], vec![first.as_str()], vec![second.as_str()]), vec!["original"]);
    assert_eq!(nested_terminal(true, true, vec![original.as_str()], vec![first.as_str()], vec![second.as_str()]), vec!["first"]);
    assert_eq!(nested_terminal(true, false, vec![original.as_str()], vec![first.as_str()], vec![second.as_str()]), vec!["second"]);
}

#[test]
fn continuing_phi_self_move_and_maybe_move_paths_keep_their_values() {
    let original = String::from("original");
    let replacement = String::from("replacement");
    let fallback = String::from("fallback");
    assert_eq!(continuing_phi(true, vec![original.as_str()], vec![replacement.as_str()], vec![fallback.as_str()]), vec!["original"]);
    assert_eq!(continuing_phi(false, vec![original.as_str()], vec![replacement.as_str()], vec![fallback.as_str()]), vec!["fallback"]);

    assert_eq!(self_move_then_reinit(false, vec![original.as_str()], vec![replacement.as_str()]), vec!["original"]);
    assert_eq!(self_move_then_reinit(true, vec![original.as_str()], vec![replacement.as_str()]), vec!["replacement"]);
    assert_eq!(maybe_move_then_reinit(false, vec![original.as_str()], vec![replacement.as_str()]), vec!["replacement"]);
    assert_eq!(maybe_move_then_reinit(true, vec![original.as_str()], vec![replacement.as_str()]), vec!["replacement"]);
}

#[test]
fn ordinary_list_and_view_rebinding_remain_unchanged() {
    assert_eq!(ordinary_list(false, vec![1], vec![2, 3]), vec![1]);
    assert_eq!(ordinary_list(true, vec![1], vec![2, 3]), vec![2, 3]);

    let original = String::from("original");
    let replacement = String::from("replacement");
    assert_eq!(ordinary_view(false, original.as_str(), replacement.as_str()), "original");
    assert_eq!(ordinary_view(true, original.as_str(), replacement.as_str()), "replacement");
}

#[test]
fn short_circuit_move_and_append_paths_restore_the_callers_view() {
    let original = String::from("original");
    for flag in [false, true] {
        assert_eq!(short_circuit_consume(flag, original.as_str()), vec!["original"]);
        assert_eq!(short_circuit_condition(flag, original.as_str()), vec!["original"]);
        assert_eq!(branch_append_restore(flag, original.as_str()), vec!["original"]);
        assert_eq!(rhs_append_restore(flag, original.as_str()), vec!["original"]);
        assert_eq!(assigned_append_restore(flag, original.as_str()), vec!["original"]);
        assert_eq!(condition_append_restore(flag, original.as_str()), vec!["original"]);
        assert_eq!(condition_append_consume(flag, original.as_str()), vec!["original"]);
        for branch in [false, true] {
            assert_eq!(short_circuit_join(flag, branch, original.as_str()), vec!["original"]);
        }
    }
    assert_eq!(append_then_restore(original.as_str()), vec!["original"]);
    assert_eq!(hygienic_restore(original.as_str()), vec!["original"]);
}

#[test]
fn moves_between_candidates_are_recorded_before_branch_joins() {
    let first = String::from("first");
    let second = String::from("second");
    for flag in [false, true] {
        for choose in [false, true] {
            let expected = if choose && !flag { "first" } else { "second" };
            assert_eq!(multiple_candidate_move(flag, choose, first.as_str(), second.as_str()), vec![expected]);
            let expected = if choose { "first" } else { "second" };
            assert_eq!(cross_candidate_move(flag, choose, first.as_str(), second.as_str()), vec![expected]);
        }
    }
}
"#;
