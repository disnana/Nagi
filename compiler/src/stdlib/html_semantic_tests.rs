//! Actual loader/checker/sealed-emitter checks. No alternative HTML registry.
use crate::{ast::Program, check, emit, source};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "nagi-sf04-semantic-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("owned fixture directory: {error}"),
            }
        }
    }
    fn load(&self, text: &str, high: bool) -> source::Sources {
        let path = self.0.join(if high { "main.nagi" } else { "main.low" });
        fs::write(&path, text).unwrap();
        // A load/parse error fails the test; it cannot satisfy a semantic negative.
        source::load(&path, high)
            .unwrap_or_else(|error| panic!("fixture did not load: {error}\n{text}"))
    }
    fn checked(&self, text: &str, high: bool) -> Program {
        let mut loaded = self.load(text, high);
        check::check(&mut loaded.program)
            .unwrap_or_else(|error| panic!("{}\n{text}", loaded.diagnostic(&error)));
        loaded.program
    }
    fn rust(&self, program: Program) -> String {
        let checked = check::finalize(
            program,
            Program::default(),
            source::SourceProvenance::user_low_unmapped(),
        )
        .unwrap();
        emit::rust(&checked).unwrap()
    }
    fn reject(&self, text: &str, high: bool, reason: &str, marker: &str) {
        let matching: Vec<_> = text
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.starts_with('#') && line.contains(marker))
            .map(|(line, _)| line + 1)
            .collect();
        assert_eq!(
            matching.len(),
            1,
            "offending marker must identify one actual input line: {marker}"
        );
        let mut loaded = self.load(text, high);
        let error = check::check(&mut loaded.program).expect_err("semantic negative was accepted");
        let diagnostic = loaded.diagnostic(&error);
        assert!(diagnostic.contains(reason), "{diagnostic}\n{text}");
        let file = if high { "main.nagi" } else { "main.low" };
        assert!(
            diagnostic.contains(&format!("{file}:{}\n", matching[0])),
            "wrong primary source line\n{diagnostic}\n{text}"
        );
    }
    fn reject_high_and_saved_low(&self, text: &str, reason: &str, marker: &str) {
        let initial = self.load(text, true);
        let low = emit::low(&initial.program);
        self.reject(text, true, reason, marker);
        self.reject(&low, false, reason, marker);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn list_detail_high_saved_and_handwritten_low_generate_checked_calls() {
    let fixture = Fixture::new();
    let high = fixture.checked(
        include_str!("../../tests/fixtures/security-sf04/list-detail.nagi"),
        true,
    );
    let saved = fixture.checked(&emit::low(&high), false);
    let manual = fixture.checked(
        include_str!("../../tests/fixtures/security-sf04/list-detail-manual.low"),
        false,
    );
    for program in [high, saved, manual] {
        let rust = fixture.rust(program);
        for call in [
            "::nagi_runtime::html::policy(",
            "::nagi_runtime::html::limits(",
            "::nagi_runtime::html::empty(",
            "::nagi_runtime::html::text(",
            "::nagi_runtime::html::attributes(",
            "::nagi_runtime::html::title(",
            "::nagi_runtime::html::href(",
            "::nagi_runtime::html::navigation(",
            "::nagi_runtime::html::element(",
            "::nagi_runtime::html::join(",
            "::nagi_runtime::html::copy_fragment(",
            "::nagi_runtime::html::document(",
            "::nagi_runtime::http_server::html_response(",
        ] {
            assert!(
                rust.contains(call),
                "missing sealed native call {call}\n{rust}"
            );
        }
        assert!(!rust.contains("::nagi_runtime::http_server::html("));
        assert!(
            !rust.contains(".clone()"),
            "HTML owned path acquired an implicit Clone\n{rust}"
        );
    }
}

#[test]
fn renamed_imports_keep_nominal_document_identity() {
    let fixture = Fixture::new();
    let text = "import std.html as markup\nimport std.http.server as web\nclass HtmlDocument:\n    value: i64\ndef respond(document: markup.HtmlDocument) -> web.Response:\n    return web.html_response(web.Status.OK, document)\n";
    let high = fixture.checked(text, true);
    let saved = fixture.checked(&emit::low(&high), false);
    let manual = fixture.checked("import std.html as markup;\nimport std.http.server as web;\nrecord HtmlDocument { value: i64; }\nfn respond(document: markup.HtmlDocument) -> web.Response {\n    return web.html_response(web.Status.OK, document);\n}\n", false);
    for program in [high, saved, manual] {
        let document = program
            .functions
            .iter()
            .find(|f| {
                f.name.ends_with("respond")
                    || crate::modules::display_symbol(&f.name).ends_with("respond")
            })
            .unwrap();
        assert_eq!(
            document.params[0].1,
            super::resource_type(super::Resource::HtmlDocument, vec![])
        );
        let rust = fixture.rust(program);
        assert!(rust.contains("::nagi_runtime::http_server::html_response"));
        assert!(rust.contains("::nagi_runtime::html::HtmlDocument"));
    }
    fixture.reject_high_and_saved_low("import std.http.server as web\nclass HtmlDocument:\n    value: i64\ndef wrong(document: HtmlDocument) -> web.Response:\n    return web.html_response(web.Status.OK, document)\n", "型", "return");
}

#[test]
fn owned_reuse_copy_and_opaque_fields_are_checker_negatives() {
    let fixture = Fixture::new();
    fixture.reject_high_and_saved_low(
        include_str!("../../tests/fixtures/security-sf04/use-after-consume.nagi"),
        "move",
        "return",
    );
    fixture.reject_high_and_saved_low(
        include_str!("../../tests/fixtures/security-sf04/builtin-copy.nagi"),
        "Clone",
        "copy(view(fragment))",
    );
    fixture.reject_high_and_saved_low(
        include_str!("../../tests/fixtures/security-sf04/opaque-field.nagi"),
        "encoded",
        "fragment.encoded",
    );
    fixture.reject_high_and_saved_low("import std.html as html\nimport std.http.server as http\ndef wrong(document: html.HtmlDocument) -> http.Response:\n    return http.html_response(http.Status.OK, view(document))\n", "型", "return");
}

#[test]
fn nested_owned_clone_serde_and_debug_are_rejected_but_shared_data_is_allowed() {
    let fixture = Fixture::new();
    fixture.reject_high_and_saved_low("import std.html as html\ndef wrong(fragments: List[List[html.HtmlFragment]]):\n    copy(view(fragments))\n", "Clone", "copy(view(fragments))");
    let fragment = super::resource_type(super::Resource::HtmlFragment, vec![]);
    for ty in [
        fragment.clone(),
        crate::ast::Type::generic("Option", vec![fragment]),
    ] {
        assert!(!crate::capabilities::debug_supported(
            &ty,
            &Default::default(),
            &Default::default()
        ));
    }
    fixture.reject_high_and_saved_low("import std.html as html\ndef wrong(fragment: html.HtmlFragment):\n    json_encode(fragment)\n", "Serialize", "json_encode(fragment)");
    fixture.reject_high_and_saved_low(
        "import std.html as html\ndef wrong():\n    json_decode[html.HtmlTag](\"{}\")\n",
        "JSON",
        "json_decode[",
    );
    let text = "import std.html as html\nclass Holder:\n    fragment: html.HtmlFragment\ndef retain(fragment: html.HtmlFragment) -> shared[Holder]:\n    return share(Holder(fragment=fragment))\ndef retain_many(fragments: List[html.HtmlFragment]) -> List[html.HtmlFragment]:\n    return fragments\ndef copy_shared(fragments: List[shared[html.HtmlFragment]]) -> List[shared[html.HtmlFragment]]:\n    return copy(view(fragments))\ndef duplicate_tag(tag: html.HtmlTag) -> html.HtmlTag:\n    return copy(view(tag))\n";
    let high = fixture.checked(text, true);
    let saved = fixture.checked(&emit::low(&high), false);
    for program in [high, saved] {
        let rust = fixture.rust(program);
        assert!(rust.contains("::nagi_runtime::html::HtmlFragment"));
        assert!(rust.contains("::nagi_runtime::html::HtmlTag"));
        assert!(
            !rust.contains("::std::fmt::Debug"),
            "owned Holder acquired Debug\n{rust}"
        );
        assert!(
            !rust.contains("#[derive("),
            "opaque Holder acquired a native derive\n{rust}"
        );
    }
}

#[test]
fn existing_http_without_html_import_keeps_local_names_and_canonical_policy() {
    let fixture = Fixture::new();
    let text = "import std.http.server as web\nclass HtmlDocument:\n    value: i64\ndef plain() -> web.Response:\n    policy = web.public_policy[i64]()\n    return web.text(web.Status.OK, \"plain\")\n";
    let high = fixture.checked(text, true);
    let saved = fixture.checked(&emit::low(&high), false);
    let manual = fixture.checked("import std.http.server as web;\nrecord HtmlDocument { value: i64; }\nfn plain() -> web.Response {\n    let policy = web.public_policy[i64]();\n    return web.text(web.Status.OK, \"plain\");\n}\n", false);
    for program in [high, saved, manual] {
        assert!(!program
            .modules
            .modules
            .iter()
            .any(|module| module.id.0 == "stdlib:std.html"));
        assert_eq!(program.classes.len(), 1);
        assert_ne!(
            program.classes[0].name,
            super::resource_type(super::Resource::HtmlDocument, vec![]).0
        );
        let rust = fixture.rust(program);
        assert!(rust.contains("::nagi_runtime::http_server::text("));
        assert!(rust.contains("::nagi_runtime::http_server::public_policy::<"));
        assert!(!rust.contains("::nagi_runtime::html::HtmlDocument"));
    }
    fixture.reject("import std.http.server as web;\nrecord HtmlDocument { value: i64; }\nfn wrong(document: HtmlDocument) -> web.Response {\n    return web.html_response(web.Status.OK, document);\n}\n", false, "型", "return");
    fixture.reject("import std.html as markup;\nfn wrong(fragments: List[List[markup.HtmlFragment]]) {\n    copy(view(fragments));\n}\n", false, "Clone", "copy(view(fragments))");
}
