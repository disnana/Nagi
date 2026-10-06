use super::*;

#[test]
fn final_factory_checks_and_seals_owned_input() {
    let program = crate::parser::parse("fn main() -> unit { print(42); }", false).unwrap();
    let checked = crate::check::finalize(
        program,
        Program::default(),
        SourceProvenance::user_low_unmapped(),
    )
    .unwrap();
    assert!(crate::emit::rust(&checked).unwrap().contains("println!"));
    assert!(checked.program().functions.iter().any(|f| f.name == "main"));
}

#[test]
fn final_factory_rejects_invalid_input() {
    let program =
        crate::parser::parse("fn main() -> unit { let value: i64 = \"wrong\"; }", false).unwrap();
    assert!(crate::check::finalize(
        program,
        Program::default(),
        SourceProvenance::user_low_unmapped()
    )
    .is_err());
}

#[test]
fn sealing_rejects_missing_expression_types() {
    let program = crate::parser::parse("fn main() -> unit { print(42); }", false).unwrap();
    let error = validate_facts(&program).unwrap_err();
    assert!(error.contains("missing checked expression type"));
}

fn checked_high(source: &str) -> CheckedProgram {
    crate::check::finalize(
        crate::parser::parse(source, true).unwrap(),
        Program::default(),
        SourceProvenance::user_low_unmapped(),
    )
    .unwrap()
}

#[test]
fn missing_name_resolution_is_rejected_without_rechecking() {
    let mut program = crate::parser::parse("fn main() -> unit { print(42); }", false).unwrap();
    crate::check::check(&mut program).unwrap();
    let S::Expr(expression) = &mut program.functions[0].body[0].kind else {
        panic!()
    };
    expression.resolution = None;
    let error = CheckedProgram::seal(program, SourceProvenance::user_low_unmapped())
        .err()
        .unwrap();
    assert_eq!(error.kind(), FailureKind::CompilerDefect);
    assert!(error
        .to_string()
        .contains("missing checked name resolution"));
}

#[test]
fn missing_declaration_type_is_rejected_without_rechecking() {
    let mut program = crate::parser::parse(
        "fn main() -> unit { let value: i64 = 42; print(value); }",
        false,
    )
    .unwrap();
    crate::check::check(&mut program).unwrap();
    let S::Assign { annotation, .. } = &mut program.functions[0].body[0].kind else {
        panic!()
    };
    *annotation = None;
    let error = CheckedProgram::seal(program, SourceProvenance::user_low_unmapped())
        .err()
        .expect("missing declaration type must not reach the emitter");
    assert_eq!(error.kind(), FailureKind::CompilerDefect);
    assert!(error
        .to_string()
        .contains("missing checked declaration type"));
}

#[test]
fn missing_binding_and_pattern_types_are_rejected_without_rechecking() {
    for source in [
        "fn main() -> unit { let value: i64 = 42; print(value); }",
        "fn main() -> unit { for item in [1, 2] { print(item); } }",
        "fn read(value: Option[i64]) -> i64 { match value { case Some(item) { return item; } case None { return 0; } } }",
    ] {
        let mut program = crate::parser::parse(source, false).unwrap();
        crate::check::check(&mut program).unwrap();
        let statement = &mut program.functions[0].body[0];
        match &mut statement.kind {
            S::Assign { .. } | S::SpawnBind { .. } | S::For(..) => statement.binding_type = None,
            S::Match(_, arms) => arms[0].pattern.bindings_mut()[0].ty = None,
            _ => panic!("wrong fault fixture"),
        }
        let error = CheckedProgram::seal(program, SourceProvenance::user_low_unmapped()).err().expect("missing binding type must not reach codegen");
        assert_eq!(error.kind(), FailureKind::CompilerDefect);
        assert!(error.to_string().contains("missing checked"));
    }
}

#[test]
fn sealed_missing_expression_plan_fails_closed() {
    let mut checked = checked_high("def main():\n    print(42)\n");
    checked.emission.functions[0].expressions.clear();
    assert!(crate::emit::rust(&checked)
        .unwrap_err()
        .contains("missing checked expression plan"));
}

#[test]
fn sealed_missing_operand_role_fails_closed() {
    let mut checked = checked_high("def restore(part: view[str]) -> List[view[str]]:\n    local = \"inner\"\n    parts = [view(local)]\n    parts = [part]\n    return parts\n");
    checked.emission.functions[0]
        .flow
        .as_mut()
        .unwrap()
        .expression_uses
        .clear();
    assert!(crate::emit::rust(&checked)
        .unwrap_err()
        .contains("missing checked operand role"));
}

#[test]
fn sealed_missing_required_flow_or_slots_fails_closed() {
    let source = "def restore(part: view[str]) -> List[view[str]]:\n    local = \"inner\"\n    parts = [view(local)]\n    parts = [part]\n    return parts\n";
    let mut missing_flow = checked_high(source);
    assert!(missing_flow.emission.functions[0].flow.is_some());
    missing_flow.emission.functions[0].flow = None;
    assert!(crate::emit::rust(&missing_flow)
        .unwrap_err()
        .contains("missing checked flow plan"));
    let mut missing_slots = checked_high(source);
    let flow = missing_slots.emission.functions[0].flow.as_mut().unwrap();
    assert!(!flow.storage_slots.is_empty());
    flow.storage_slots.clear();
    assert!(crate::emit::rust(&missing_slots)
        .unwrap_err()
        .contains("missing checked storage slots"));
}

#[test]
fn facts_and_emission_are_deterministic_and_read_only() {
    let source = "def restore(part: view[str]) -> List[view[str]]:\n    local = \"inner\"\n    parts = [view(local)]\n    parts = [part]\n    return parts\n";
    let first = checked_high(source);
    let second = checked_high(source);
    let before = crate::emit::low(first.program());
    assert_eq!(first.emission.classes, second.emission.classes);
    assert_eq!(first.emission.rust_types, second.emission.rust_types);
    assert_eq!(
        first.emission.functions[0].body,
        second.emission.functions[0].body
    );
    assert_eq!(
        first.emission.functions[0].expressions,
        second.emission.functions[0].expressions
    );
    assert_eq!(
        format!("{:?}", first.emission.functions[0].flow),
        format!("{:?}", second.emission.functions[0].flow)
    );
    assert_eq!(
        crate::emit::rust(&first).unwrap(),
        crate::emit::rust(&second).unwrap()
    );
    assert_eq!(crate::emit::low(first.program()), before);
    let saved = crate::parser::parse(&before, false).unwrap();
    let saved = crate::check::finalize(
        saved,
        Program::default(),
        SourceProvenance::user_low_unmapped(),
    )
    .unwrap();
    // Saved Low has its own token and line IDs. Compare decisions while
    // normalizing only the private storage identifiers derived from those IDs.
    let normalize = |checked: &CheckedProgram| {
        let mut rust = crate::emit::rust(checked).unwrap();
        for (index, name) in checked.emission.functions[0]
            .flow
            .as_ref()
            .unwrap()
            .storage_slots
            .iter()
            .enumerate()
        {
            rust = rust.replace(name, &format!("__checked_slot_{index}"));
        }
        rust
    };
    assert_eq!(first.emission.rust_types, saved.emission.rust_types);
    assert_eq!(
        first.emission.functions[0].body,
        saved.emission.functions[0].body
    );
    assert_eq!(normalize(&first), normalize(&saved));
}

#[test]
fn symbolic_enum_owner_is_not_an_untyped_payload() {
    let program = crate::parser::parse(
        "enum Color { Red; } fn value() -> Color { return Color.Red; }",
        false,
    )
    .unwrap();
    let checked = crate::check::finalize(
        program,
        Program::default(),
        SourceProvenance::user_low_unmapped(),
    )
    .unwrap();
    assert!(crate::emit::rust(&checked).unwrap().contains("Color::Red"));
}

struct Files(std::path::PathBuf);
impl Files {
    fn new() -> Self {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "nagi-checked-provenance-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) -> std::path::PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn generated_saved_native_replacement_and_synthetic_origins_are_distinct() {
    use crate::source::{self, LoweringKind};
    let files = Files::new();
    let high_path = files.write(
        "main.nagi",
        "def value() -> i64:\n    return 1\ndef other() -> i64:\n    return 2\n",
    );
    let mut sources = source::load(&high_path, true).unwrap();
    let mut high = sources.program.clone();
    crate::check::check(&mut high).unwrap();
    let low = crate::emit::low_with_lines(&high);
    let saved_path = files.write("saved.low", &low.text);
    let saved = source::load(&saved_path, false).unwrap();
    let saved_origin = saved.provenance().origin(saved.program.functions[0].line);
    assert_eq!(saved_origin.kind, LoweringKind::UserLow);
    assert_eq!(
        saved_origin.path,
        Some(std::fs::canonicalize(&saved_path).unwrap())
    );
    let mut primary = crate::parser::parse(&low.text, false).unwrap();
    low.restore_lines(&mut primary).unwrap();
    let native_path=files.write("native.low","fn ordinary() -> i64 { return 7; } @replace(\"generated::value\"); fn replacement() -> i64 { return 3; } @replace(\"generated::other\"); fn replacement2() -> i64 { return 4; }\n");
    let native = sources.append(source::load(&native_path, false).unwrap());
    let checked = crate::check::finalize(primary, native, sources.provenance()).unwrap();
    assert_eq!(
        checked.provenance().origin(1).kind,
        LoweringKind::GeneratedLow
    );
    assert_eq!(checked.provenance().origin(0).kind, LoweringKind::Unknown);
    assert_eq!(
        checked.provenance().origin(usize::MAX).kind,
        LoweringKind::Unknown
    );
    let generated = crate::emit::rust_with_lines(&checked).unwrap();
    let emitted_line = |needle: &str| {
        generated
            .text
            .lines()
            .position(|line| line.contains(needle))
            .unwrap()
            + 1
    };
    let ordinary = generated.line_provenance(emitted_line("return 7"));
    assert_eq!(ordinary.kind, LoweringKind::NativeLow);
    assert_eq!(ordinary.replacement_target, None);
    let replacement = generated.line_provenance(emitted_line("return 3"));
    let replacement2 = generated.line_provenance(emitted_line("return 4"));
    assert_eq!(replacement.kind, LoweringKind::Replacement);
    assert_eq!(replacement2.kind, LoweringKind::Replacement);
    assert_eq!(
        replacement.path,
        Some(std::fs::canonicalize(&native_path).unwrap())
    );
    assert_eq!(replacement.line, Some(1));
    assert_ne!(
        replacement.replacement_target,
        replacement2.replacement_target
    );
    assert_eq!(generated.line_provenance(1).kind, LoweringKind::Synthetic);
    assert_eq!(
        generated.line_provenance(usize::MAX).kind,
        LoweringKind::Synthetic
    );
    let rust_file = files.0.join("generated.rs");
    for (needle, label) in [("return 7", "native Low"), ("return 3", "native @replace")] {
        let message=serde_json::json!({"reason":"compiler-message","target":{"src_path":rust_file},"message":{"level":"error","message":"adapter mismatch","code":{"code":"E0308"},"rendered":"original Rust diagnostic\n","spans":[{"file_name":rust_file,"line_start":emitted_line(needle),"is_primary":true,"label":"cause"}],"children":[]}}).to_string();
        let text = crate::diagnostics::cargo_message_with_details(
            &message, &generated, &rust_file, &sources, false,
        )
        .unwrap();
        assert!(text.contains(&format!("emission origin: {label}")));
        assert!(!text.contains("compiler defect"));
        assert!(text.contains("native.low:1"));
    }
    let native_rust = files.0.join("adapter.rs");
    let message=serde_json::json!({"reason":"compiler-message","target":{"src_path":rust_file},"message":{"level":"error","message":"native failure","rendered":"native Rust original\n","spans":[{"file_name":native_rust,"line_start":1,"is_primary":true}],"children":[]}}).to_string();
    assert_eq!(
        crate::diagnostics::cargo_message_with_details(
            &message, &generated, &rust_file, &sources, false
        )
        .unwrap(),
        "native Rust original\n"
    );
}

#[test]
fn unknown_origin_is_not_user_blame() {
    let unknown = FinalizeError::checked(
        "unlocated error".into(),
        &SourceProvenance::user_low_unmapped(),
        false,
    );
    assert_eq!(unknown.kind(), FailureKind::Unclassified);
    assert_eq!(
        SourceProvenance::user_low_unmapped().origin(42).kind,
        crate::source::LoweringKind::UserLow
    );
}

#[test]
fn sealed_missing_rust_type_plan_fails_closed() {
    let mut checked = checked_high("def main():\n    print(42)\n");
    checked.emission.rust_types.clear();
    assert!(crate::emit::rust(&checked)
        .unwrap_err()
        .contains("missing checked Rust type plan"));
}

#[test]
fn final_diagnostic_transports_kind_without_changing_user_messages() {
    use crate::source::{self, LoweringKind};
    let files = Files::new();
    let path = files.write("main.nagi", "def main():\n    print(42)\n");
    let sources = source::load(&path, true).unwrap();
    assert_eq!(
        sources.provenance().origin(1).kind,
        LoweringKind::GeneratedLow
    );
    let message = "line 2: missing checked condition".to_owned();
    let generated = FinalizeError::checked(message.clone(), &sources.provenance(), false);
    assert_eq!(generated.kind(), FailureKind::CompilerDefect);
    assert!(crate::diagnostics::finalize_message(&generated, &sources)
        .contains("compiler defect / ICE candidate"));
    let mixed = FinalizeError::checked(message.clone(), &sources.provenance(), true);
    assert_eq!(mixed.kind(), FailureKind::Unclassified);
    assert!(crate::diagnostics::finalize_message(&mixed, &sources)
        .contains("unclassified final-check failure"));
    let user = FinalizeError::checked(
        message.clone(),
        &SourceProvenance::user_low_unmapped(),
        false,
    );
    assert_eq!(user.kind(), FailureKind::UserError);
    assert_eq!(
        crate::diagnostics::finalize_message(&user, &sources),
        sources.diagnostic(&message)
    );
}

#[test]
fn task_sealing_rejects_missing_or_mismatched_scope_and_use_facts() {
    let checked = checked_high("async def work() -> i64:\n    return 7\nasync def main() -> Result[unit, Error]:\n    async with scope:\n        task = spawn work()\n        received = await task\n    return ok(print(0))\n");
    for mutation in 0..4 {
        let mut program = checked.program().clone();
        let main = program
            .functions
            .iter_mut()
            .find(|f| f.name == "main")
            .unwrap();
        let scope = &mut main.body[0];
        if mutation == 0 {
            scope.task.bridge = false;
        }
        let S::Scope(body) = &mut scope.kind else {
            panic!()
        };
        if mutation == 1 {
            body[0].task.scope = None;
        }
        if mutation == 2 {
            body[1].task.uses.clear();
        }
        if mutation == 3 {
            body[1].task.uses[0].action = TaskAction::Discard;
        }
        let error = CheckedProgram::seal(program, SourceProvenance::user_low_unmapped())
            .err()
            .expect("invalid checked Task facts must not reach emission");
        assert_eq!(error.kind(), FailureKind::CompilerDefect);
    }
}
