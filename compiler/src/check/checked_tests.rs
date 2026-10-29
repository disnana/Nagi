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
fn item_debug_eligibility_and_identity_are_sealed() {
    let source = "class Row:\n    value: i64\ndef main():\n    print(1)\n";
    let mut checked = checked_high(source);
    assert!(checked.emission.classes[0].debug);
    checked.emission.classes[0].debug = false;
    assert!(checked
        .validate_for_emission()
        .unwrap_err()
        .contains("item Debug plan"));
    let mut checked = checked_high(source);
    checked.emission.classes[0].identity = "WrongRow".into();
    assert!(checked
        .validate_for_emission()
        .unwrap_err()
        .contains("item Debug plan"));
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
    let error = CheckedProgram::seal_rechecked(program, SourceProvenance::user_low_unmapped())
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
    let error = CheckedProgram::seal_rechecked(program, SourceProvenance::user_low_unmapped())
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
        let error = CheckedProgram::seal_rechecked(program, SourceProvenance::user_low_unmapped()).err().expect("missing binding type must not reach codegen");
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
        let error = CheckedProgram::seal_rechecked(program, SourceProvenance::user_low_unmapped())
            .err()
            .expect("invalid checked Task facts must not reach emission");
        assert_eq!(error.kind(), FailureKind::CompilerDefect);
    }
}

#[test]
fn final_future_capture_facts_reject_identity_argument_owner_and_span_tampering() {
    let mut program = crate::parser::parse("async def child(value: i64) -> i64:\n    return value\nasync def work(value: i64) -> i64:\n    callback = child\n    return await callback(value)\n", true).unwrap();
    let facts = super::super::check_mode(&mut program, false).unwrap();
    assert_eq!(
        facts.functions["work"].values().next().unwrap().callee,
        "child"
    );
    for mutation in 0..7 {
        let mut facts = facts.clone();
        let captures = facts.functions.get_mut("work").unwrap();
        if mutation == 0 {
            captures.clear();
        } else {
            let capture = captures.values_mut().next().unwrap();
            match mutation {
                1 => capture.callee = "wrong".into(),
                2 => capture.arguments[0].index = 1,
                3 => capture.arguments[0].owners[0].1.token += 1,
                4 => capture.arguments[0].expression.end += 1,
                5 => capture.arguments[0].passing = crate::stdlib::Passing::Borrow,
                6 => {
                    let (mut key, value) = captures.pop_first().unwrap();
                    key.start += 1;
                    captures.insert(key, value);
                }
                _ => unreachable!(),
            }
        }
        let error = CheckedProgram::seal(
            program.clone(),
            SourceProvenance::user_low_unmapped(),
            facts,
        )
        .err()
        .unwrap();
        assert_eq!(error.kind(), FailureKind::CompilerDefect);
        assert!(error.to_string().contains("Future capture facts"));
    }
}

fn user_capture(argument: &FutureArgument) -> &UserAsyncArgumentFacts {
    let ArgumentCaptureFacts::UserAsync(facts) = &argument.capture else {
        panic!("expected user async argument facts")
    };
    facts
}

fn local_binding(program: &Program, function: &str, name: &str) -> BindingId {
    let function = program
        .functions
        .iter()
        .find(|f| f.name == function)
        .unwrap();
    function
        .body
        .iter()
        .find_map(|statement| match &statement.kind {
            S::Assign { name: local, .. } if local == name => Some(BindingId {
                line: statement.line,
                token: statement.binding_span.unwrap().start,
            }),
            _ => None,
        })
        .unwrap()
}

fn parameter_binding(program: &Program, function: &str, index: usize) -> BindingId {
    let function = program
        .functions
        .iter()
        .find(|f| f.name == function)
        .unwrap();
    BindingId {
        line: function.line,
        token: function.parameter_spans[index].start,
    }
}

fn capture_entries(source: &str, manual: &str, mut verify: impl FnMut(&CheckedProgram)) {
    let high = checked_high(source);
    verify(&high);
    let low = crate::emit::low(high.program());
    // Facts are reconstructed, not transported in Low text. Its expression
    // and binding identities belong to this independent parse.
    for text in [&low, manual] {
        let checked = crate::check::finalize(
            crate::parser::parse(text, false).unwrap(),
            Program::default(),
            SourceProvenance::user_low_unmapped(),
        )
        .unwrap();
        verify(&checked);
        crate::emit::rust(&checked).unwrap();
    }
}

#[test]
fn user_async_capture_snapshots_direct_callback_alias_field_parameter_and_transfer() {
    let high = "class Pair:\n    text: str\nasync def read(part: view[str]) -> i64:\n    return len(part)\nasync def value(number: i64, text: str):\n    print(number)\nasync def work(part: view[str]):\n    owner = \"owner\"\n    alias = view(owner)\n    callback = read\n    print(await read(alias))\n    print(await callback(alias))\n    pair = Pair(text=\"field\")\n    print(await read(view(pair.text)))\n    print(await read(part))\n    print(await read(view(\"temporary\")))\n    await value(42, \"owned\")\n";
    let manual = "record Pair { text: str; }\nasync fn read(part: view[str]) -> i64 { return len(part); }\nasync fn value(number: i64, text: str) -> unit { print(number); }\nasync fn work(part: view[str]) -> unit {\nlet owner: str = \"owner\";\nlet alias: view[str] = view(owner);\nlet callback: fn[view[str], Future[i64]] = read;\nprint(await read(alias));\nprint(await callback(alias));\nlet pair: Pair = Pair(text=\"field\");\nprint(await read(view(pair.text)));\nprint(await read(part));\nprint(await read(view(\"temporary\")));\nawait value(42, \"owned\");\n}";
    capture_entries(high, manual, |checked| {
        let program = checked.program();
        let captures = &checked.final_check_facts().functions["work"];
        assert_eq!(captures.len(), 6);
        let entries: Vec<_> = captures.iter().collect();
        let owner = local_binding(program, "work", "owner");
        let alias = local_binding(program, "work", "alias");
        for (index, resolution) in [NameResolution::Function, NameResolution::Local]
            .into_iter()
            .enumerate()
        {
            let (call, capture) = entries[index];
            assert_eq!(capture.callee, "read");
            assert_eq!(capture.resolution, resolution);
            let argument = &capture.arguments[0];
            let facts = user_capture(argument);
            assert_eq!(
                facts.slot,
                ArgumentCaptureSlot {
                    call: *call,
                    argument: 0
                }
            );
            assert_eq!(
                facts.storage,
                CaptureStorageId {
                    call: *call,
                    argument: 0
                }
            );
            assert_eq!(facts.transfer, ExprUseMode::Copy);
            assert!(!facts.views_absent);
            assert_eq!(facts.release, ArgumentRelease::FutureInvocationExit);
            assert_eq!(facts.depths.len(), 1);
            assert_eq!(
                facts.depths[0].origins,
                vec![FlowOrigin {
                    binding: owner,
                    fields: vec![],
                    owner_loan: true,
                    static_origin: false,
                }]
            );
            assert!(!facts.depths[0].borrows_temporary);
            assert_eq!(argument.owners.len(), 1);
            assert_eq!(argument.owners[0].1, alias);
        }
        let field = &user_capture(&entries[2].1.arguments[0]).depths[0];
        assert_eq!(
            field.origins,
            vec![FlowOrigin {
                binding: local_binding(program, "work", "pair"),
                fields: vec!["text".into()],
                owner_loan: true,
                static_origin: false,
            }]
        );
        let parameter = &user_capture(&entries[3].1.arguments[0]).depths[0];
        assert_eq!(
            parameter.origins,
            vec![FlowOrigin {
                binding: parameter_binding(program, "work", 0),
                fields: vec![],
                owner_loan: false,
                static_origin: false,
            }]
        );
        let temporary = user_capture(&entries[4].1.arguments[0]);
        assert!(temporary.depths[0].origins.is_empty());
        assert!(temporary.depths[0].borrows_temporary);
        assert!(!temporary.views_absent);
        assert_eq!(temporary.release, ArgumentRelease::FutureInvocationExit);
        let copy = user_capture(&entries[5].1.arguments[0]);
        assert_eq!(copy.transfer, ExprUseMode::Copy);
        assert!(copy.depths.is_empty());
        assert!(copy.views_absent);
        assert_eq!(copy.release, ArgumentRelease::NotRequired);
        let moved = user_capture(&entries[5].1.arguments[1]);
        assert_eq!(moved.transfer, ExprUseMode::Move);
        assert!(moved.depths.is_empty());
        assert!(moved.views_absent);
        assert_eq!(moved.release, ArgumentRelease::FutureInvocationExit);
    });
}

#[test]
fn user_async_capture_depths_preserve_container_loans_separately_from_contents() {
    let high = "async def read(parts: view[view[view[str]]]) -> i64:\n    return len(parts)\nasync def work():\n    owner = \"owner\"\n    parts = [view(owner)]\n    rows = [view(parts)]\n    print(await read(view(rows)))\n";
    let manual = "async fn read(parts: view[view[view[str]]]) -> i64 { return len(parts); }\nasync fn work() -> unit {\nlet owner: str = \"owner\";\nlet parts: List[view[str]] = [view(owner)];\nlet rows: List[view[view[str]]] = [view(parts)];\nprint(await read(view(rows)));\n}";
    capture_entries(high, manual, |checked| {
        let captures = &checked.final_check_facts().functions["work"];
        let facts = user_capture(&captures.values().next().unwrap().arguments[0]);
        assert_eq!(facts.depths.len(), 3);
        let owner = local_binding(checked.program(), "work", "owner");
        let parts = local_binding(checked.program(), "work", "parts");
        let rows = local_binding(checked.program(), "work", "rows");
        for (depth, expected) in [vec![owner, parts, rows], vec![owner, parts], vec![owner]]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                facts.depths[depth]
                    .origins
                    .iter()
                    .map(|origin| origin.binding)
                    .collect::<Vec<_>>(),
                expected
            );
            assert!(!facts.depths[depth].borrows_temporary);
        }
        assert_eq!(facts.release, ArgumentRelease::Deferred);
    });
    let checked = checked_high(&format!("{high}async def main():\n    await work()\n"));
    let error =
        crate::go_backend::source(&checked, &[], crate::project::BuildProfile::Dev).unwrap_err();
    assert!(error.starts_with("line 1: Go backend:"), "{error}");
    assert!(
        error.contains("type view[view[view[str]]] is not supported"),
        "{error}"
    );
}

#[test]
fn user_async_capture_earlier_argument_does_not_observe_later_content_mutation() {
    let high = "async def read(part: view[str], ignored: unit) -> i64:\n    return len(part)\nasync def work(first: view[str], second: view[str]):\n    parts = [first]\n    print(await read(parts[0], append(parts, second)))\n";
    let manual = "async fn read(part: view[str], ignored: unit) -> i64 { return len(part); }\nasync fn work(first: view[str], second: view[str]) -> unit {\nlet parts: List[view[str]] = [first];\nprint(await read(parts[0], append(parts, second)));\n}";
    capture_entries(high, manual, |checked| {
        let capture = checked.final_check_facts().functions["work"]
            .values()
            .next()
            .unwrap();
        let before = user_capture(&capture.arguments[0]);
        assert_eq!(
            before.depths[0]
                .origins
                .iter()
                .map(|origin| origin.binding)
                .collect::<Vec<_>>(),
            vec![parameter_binding(checked.program(), "work", 0)]
        );
        assert_eq!(
            user_capture(&capture.arguments[1]).release,
            ArgumentRelease::NotRequired
        );
    });
}

#[test]
fn user_async_capture_rechecks_replace_facts_with_final_loop_pass() {
    let source = "async def read(part: view[str]) -> i64:\n    return len(part)\nasync def work(first: view[str], second: view[str], count: i64):\n    selected = first\n    for index in range(count):\n        print(await read(selected))\n        selected = second\n";
    let manual = "async fn read(part: view[str]) -> i64 { return len(part); }\nasync fn work(first: view[str], second: view[str], count: i64) -> unit {\nlet selected: view[str] = first;\nfor index in range(count) { print(await read(selected)); selected = second; }\n}";
    capture_entries(source, manual, |checked| {
        let mut program = checked.program().clone();
        let first = super::super::check_mode(&mut program, false).unwrap();
        let second = super::super::check_mode(&mut program, false).unwrap();
        assert_eq!(first, second);
        assert_eq!(&first, checked.final_check_facts());
        assert_eq!(first.functions["work"].len(), 1);
        let capture = first.functions["work"].values().next().unwrap();
        let origins = &user_capture(&capture.arguments[0]).depths[0].origins;
        assert_eq!(
            origins
                .iter()
                .map(|origin| origin.binding)
                .collect::<Vec<_>>(),
            vec![
                parameter_binding(&program, "work", 0),
                parameter_binding(&program, "work", 1)
            ]
        );
    });
}

#[test]
fn user_async_capture_proven_absence_still_defers_view_containing_release() {
    let high = "async def read(parts: List[view[str]], optional: Option[view[str]]):\n    print(len(parts))\nasync def work():\n    await read([], None)\n";
    let manual = "async fn read(parts: List[view[str]], optional: Option[view[str]]) -> unit { print(len(parts)); }\nasync fn work() -> unit { await read([], None); }";
    capture_entries(high, manual, |checked| {
        let capture = checked.final_check_facts().functions["work"]
            .values()
            .next()
            .unwrap();
        for argument in &capture.arguments {
            let facts = user_capture(argument);
            assert!(facts.views_absent);
            assert_eq!(facts.release, ArgumentRelease::Deferred);
            assert!(!facts.depths.is_empty());
            assert!(facts
                .depths
                .iter()
                .all(|depth| depth.origins.is_empty() && !depth.borrows_temporary));
        }
        assert_eq!(
            user_capture(&capture.arguments[0]).transfer,
            ExprUseMode::Move
        );
        assert_eq!(
            user_capture(&capture.arguments[1]).transfer,
            ExprUseMode::Copy
        );
    });
}

#[test]
fn user_async_capture_preserves_explicit_static_origin_and_standard_is_unplanned() {
    let files = Files::new();
    let path = files.write("main.nagi", "import std.http.server as http\nimport std.actor as actor\nasync def read(part: view[str]) -> i64:\n    return len(part)\nasync def main():\n    print(await read(http.Status.OK.phrase))\n    await sleep(0)\n    await actor.yield_now()\n");
    let loaded = crate::source::load(&path, true).unwrap();
    let provenance = loaded.provenance();
    let checked = crate::check::finalize(loaded.program, Program::default(), provenance).unwrap();
    let captures = checked
        .final_check_facts()
        .functions
        .values()
        .find(|captures| !captures.is_empty())
        .unwrap();
    assert_eq!(captures.len(), 3);
    let entries: Vec<_> = captures.values().collect();
    let facts = user_capture(&entries[0].arguments[0]);
    assert_eq!(
        facts.depths[0].origins,
        vec![FlowOrigin {
            binding: BindingId { line: 0, token: 0 },
            fields: vec![],
            owner_loan: false,
            static_origin: true,
        }]
    );
    assert!(!facts.depths[0].borrows_temporary);
    assert!(!facts.views_absent);
    assert_eq!(facts.release, ArgumentRelease::FutureInvocationExit);
    assert_eq!(entries[1].resolution, NameResolution::Builtin);
    assert_eq!(
        entries[1].arguments[0].capture,
        ArgumentCaptureFacts::UnplannedStandard
    );
    assert_eq!(entries[2].resolution, NameResolution::Standard);
    assert!(entries[2].arguments.is_empty());
}

#[test]
fn user_async_capture_plain_graph_release_and_storage_are_rebuilt() {
    let high = "class Payload:\n    text: str\n    numbers: List[i64]\nasync def take(payload: Payload, nested: List[Option[str]], result: Result[str, Error]):\n    print(payload.text)\nasync def main():\n    await take(Payload(text=\"owned\", numbers=[1]), [some(\"nested\")], ok(\"result\"))\n";
    let manual = "record Payload { text: str; numbers: List[i64]; }\nasync fn take(payload: Payload, nested: List[Option[str]], result: Result[str, Error]) -> unit { print(payload.text); }\nasync fn main() -> unit { await take(Payload(text=\"owned\", numbers=[1]), [some(\"nested\")], ok(\"result\")); }";
    capture_entries(high, manual, |checked| {
        let (call, capture) = checked.final_check_facts().functions["main"]
            .iter()
            .next()
            .unwrap();
        let mut storage = BTreeSet::new();
        for (index, argument) in capture.arguments.iter().enumerate() {
            let facts = user_capture(argument);
            assert_eq!(facts.release, ArgumentRelease::FutureInvocationExit);
            assert_eq!(
                facts.storage,
                CaptureStorageId {
                    call: *call,
                    argument: index
                }
            );
            assert!(storage.insert(facts.storage));
        }
        crate::go_backend::source(checked, &[], crate::project::BuildProfile::Dev).unwrap();
    });
}

#[test]
fn user_async_capture_resource_and_shared_graphs_remain_deferred_even_when_copy() {
    let files = Files::new();
    let path = files.write("main.nagi", "import std.http.server as http\nclass Wrapped:\n    status: http.Status\nasync def take(value: http.Status, nested: List[http.Status], wrapped: Wrapped, handle: shared[str]):\n    print(0)\nasync def main():\n    await take(http.Status.OK, [http.Status.OK], Wrapped(status=http.Status.OK), share(\"owner\"))\n");
    let loaded = crate::source::load(&path, true).unwrap();
    let provenance = loaded.provenance();
    let checked = crate::check::finalize(loaded.program, Program::default(), provenance).unwrap();
    let capture = checked
        .final_check_facts()
        .functions
        .values()
        .flat_map(|calls| calls.values())
        .find(|capture| capture.arguments.len() == 4)
        .unwrap();
    for argument in &capture.arguments {
        assert_eq!(user_capture(argument).release, ArgumentRelease::Deferred);
    }
    assert_eq!(
        user_capture(&capture.arguments[0]).transfer,
        ExprUseMode::Copy
    );
    assert_eq!(
        user_capture(&capture.arguments[2]).transfer,
        ExprUseMode::Copy
    );
    crate::emit::rust(&checked).unwrap();
}

#[test]
fn go_user_capture_rejects_missing_storage_and_deferred_without_type_inference() {
    for mutation in 0..4 {
        let mut checked = checked_high("async def take(value: str):\n    print(value)\nasync def main():\n    await take(\"owned\")\n");
        let captures = checked.final_check_facts.functions.get_mut("main").unwrap();
        if mutation == 0 {
            captures.clear();
        } else {
            let argument = &mut captures.values_mut().next().unwrap().arguments[0];
            let ArgumentCaptureFacts::UserAsync(facts) = &mut argument.capture else {
                panic!()
            };
            match mutation {
                1 => facts.storage.argument += 1,
                2 => facts.release = ArgumentRelease::Deferred,
                3 => argument.capture = ArgumentCaptureFacts::UnplannedStandard,
                _ => unreachable!(),
            }
        }
        let error = crate::go_backend::source(&checked, &[], crate::project::BuildProfile::Dev)
            .unwrap_err();
        assert!(
            error.contains(if mutation == 2 {
                "deferred user Future capture release"
            } else {
                "internal compiler error:"
            }),
            "{error}"
        );
    }
}

#[test]
fn user_async_capture_seal_rejects_each_new_fact_mutation() {
    let mut program = crate::parser::parse("async def read(part: view[str]) -> i64:\n    return len(part)\nasync def work(part: view[str]) -> i64:\n    return await read(part)\n", true).unwrap();
    let original = super::super::check_mode(&mut program, false).unwrap();
    for mutation in 0..16 {
        let mut facts = original.clone();
        let argument = &mut facts
            .functions
            .get_mut("work")
            .unwrap()
            .values_mut()
            .next()
            .unwrap()
            .arguments[0];
        let ArgumentCaptureFacts::UserAsync(capture) = &mut argument.capture else {
            panic!()
        };
        match mutation {
            0 => capture.slot.call.start += 1,
            1 => capture.slot.argument += 1,
            2 => capture.transfer = ExprUseMode::Move,
            3 => capture.depths[0].origins[0].binding.token += 1,
            4 => capture.depths[0].origins[0].fields.push("wrong".into()),
            5 => capture.depths[0].origins[0].owner_loan = true,
            6 => capture.depths[0].origins[0].static_origin = true,
            7 => capture.depths[0].borrows_temporary = true,
            8 => capture.depths.push(capture.depths[0].clone()),
            9 => capture.views_absent = true,
            10 => capture.release = ArgumentRelease::NotRequired,
            11 => capture.transfer = ExprUseMode::Borrow,
            12 => argument.capture = ArgumentCaptureFacts::UnplannedStandard,
            13 => capture.storage.call.start += 1,
            14 => capture.storage.argument += 1,
            15 => capture.release = ArgumentRelease::Deferred,
            _ => unreachable!(),
        }
        let error = CheckedProgram::seal(
            program.clone(),
            SourceProvenance::user_low_unmapped(),
            facts,
        )
        .err()
        .unwrap();
        assert_eq!(error.kind(), FailureKind::CompilerDefect);
        assert!(
            error.to_string().contains("Future capture facts"),
            "{error}"
        );
    }
}

#[test]
fn sqlite_literal_plan_rejects_missing_index_and_payload_tampering() {
    let fixture = Files::new();
    let path = fixture.0.join("sqlite.nagi");
    std::fs::write(&path, "import std.db.sqlite as sqlite\nasync def work(tx: sqlite.Tx) -> Result[i64, sqlite.Failure]:\n    return await sqlite.exec(tx, sqlite.literal(\"DELETE FROM items\"), sqlite.parameters())\n").unwrap();
    let loaded = crate::source::load(&path, true).unwrap();
    let provenance = loaded.provenance();
    let checked = crate::check::finalize(loaded.program, Program::default(), provenance).unwrap();
    let standard_arguments: Vec<_> = checked
        .final_check_facts()
        .functions
        .values()
        .flat_map(|captures| captures.values())
        .filter(|capture| capture.resolution == NameResolution::Standard)
        .flat_map(|capture| &capture.arguments)
        .collect();
    assert_eq!(standard_arguments.len(), 3);
    assert!(standard_arguments
        .iter()
        .all(|argument| { argument.capture == ArgumentCaptureFacts::UnplannedStandard }));
    for mutation in 0..3 {
        let mut emission = EmissionPlan::build(checked.program(), checked.provenance()).unwrap();
        let sql = emission
            .functions
            .iter_mut()
            .flat_map(|f| f.expressions.values_mut())
            .find_map(|p| {
                if let Some(OperationPlan::NativeCall { sql: Some(_), .. }) = &p.operation {
                    if let Some(OperationPlan::NativeCall { sql, .. }) = &mut p.operation {
                        return Some(sql);
                    }
                }
                None
            })
            .unwrap();
        match mutation {
            0 => *sql = None,
            1 => sql.as_mut().unwrap().index = 2,
            2 => sql.as_mut().unwrap().literal = "DELETE FROM other_items".into(),
            _ => unreachable!(),
        }
        assert!(emission
            .validate()
            .unwrap_err()
            .contains("SQL argument plan"));
    }
}
