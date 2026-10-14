use nagic::{check, emit, parser, source};
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
                "nagi-sf06-compiler-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("owned fixture creation: {e}"),
            }
        }
    }
    fn checked(&self, name: &str, text: &str) -> source::Sources {
        fs::write(self.0.join(name), text).unwrap();
        let mut loaded = source::load(&self.0.join(name), name.ends_with(".nagi"))
            .expect("parse/name resolution must succeed before check");
        check::check(&mut loaded.program).unwrap_or_else(|e| panic!("{}", loaded.diagnostic(&e)));
        loaded
    }
    fn three_forms(&self, high: &str, low: &str) {
        let loaded = self.checked("main.nagi", high);
        let generated = emit::low(&loaded.program);
        fs::remove_file(self.0.join("main.nagi")).unwrap();
        self.checked("saved.low", &generated);
        self.checked("manual.low", low);
        let mut direct = parser::parse(&generated, false).unwrap();
        check::check(&mut direct).unwrap();
    }
    fn rejected(&self, name: &str, text: &str, line: usize, reason: &str) {
        fs::write(self.0.join(name), text).unwrap();
        let mut loaded = source::load(&self.0.join(name), name.ends_with(".nagi"))
            .expect("negative must parse and resolve before check");
        let saved_projection = name
            .ends_with(".nagi")
            .then(|| emit::low_with_lines(&loaded.program));
        let error =
            check::check(&mut loaded.program).expect_err("invalid SF06 program passed checker");
        let diagnostic = loaded.diagnostic(&error);
        assert!(diagnostic.contains(reason), "wrong reason: {diagnostic}");
        assert!(
            diagnostic.contains(&format!("{name}:{line}")),
            "wrong source line: {diagnostic}"
        );

        if let Some(generated) = saved_projection {
            // Invalid High has no accepted generated program. Recheck its
            // resolved Low projection with the existing source-line map.
            let mut mapped = parser::parse(&generated.text, false).expect("Low projection parse");
            generated
                .restore_lines(&mut mapped)
                .expect("exact generated origins");
            let error = check::check(&mut mapped).expect_err("mapped Low bypassed checker");
            let diagnostic = loaded.diagnostic(&error);
            assert!(
                diagnostic.contains(reason),
                "wrong mapped Low reason: {diagnostic}"
            );
            assert!(
                diagnostic.contains(&format!("{name}:{line}")),
                "wrong mapped original line: {diagnostic}"
            );
            fs::write(self.0.join("negative-projection.low"), &generated.text).unwrap();
            fs::remove_file(self.0.join(name)).unwrap();
            let mut saved = source::load(&self.0.join("negative-projection.low"), false)
                .expect("saved negative Low must parse/resolve without High");
            let error = check::check(&mut saved.program).expect_err("saved Low bypassed checker");
            let diagnostic = saved.diagnostic(&error);
            assert!(
                diagnostic.contains(reason),
                "wrong saved Low reason: {diagnostic}"
            );
            let emitted_line: usize = error
                .strip_prefix("line ")
                .unwrap()
                .split(':')
                .next()
                .unwrap()
                .parse()
                .unwrap();
            let location = saved
                .location(emitted_line)
                .expect("actual saved Low source location");
            assert_eq!(
                location.path.file_name().unwrap(),
                "negative-projection.low"
            );
            assert!(
                diagnostic.contains(&format!("negative-projection.low:{}", location.line)),
                "wrong saved Low line: {diagnostic}"
            );
            assert_eq!(
                generated.line_origin(location.line),
                Some(line),
                "saved Low rejection must originate at the same High statement"
            );
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn owned_response_body_views_and_transfer_check_in_three_forms() {
    Fixture::new().three_forms(
        "import std.http.client as client\ndef read(response: view[client.Response]) -> view[bytes]:\n    return client.body(response)\ndef take(response: client.Response) -> bytes:\n    return client.take_body(response)\ndef main():\n    print(0)\n",
        "import std.http.client as client\nfn read(response: view[client.Response]) -> view[bytes] { return client.body(response); }\nfn take(response: client.Response) -> bytes { return client.take_body(response); }\nfn main() { print(0); }\n",
    );
}

#[test]
fn actual_grant_protected_operation_checks_in_three_forms() {
    Fixture::new().three_forms(
        "import std.http.client as client\nimport std.auth as auth\nclass State:\n    n: i64\nclass Permission:\n    n: i64\nasync def call(operation: view[client.ProtectedOperation[State, Permission, i64]], grant: auth.Grant[Permission], input: i64) -> Result[client.Response, client.Failure]:\n    return await client.execute(operation, grant, input)\ndef main():\n    print(0)\n",
        "import std.http.client as client\nimport std.auth as auth\nrecord State { n: i64; }\nrecord Permission { n: i64; }\nasync fn call(operation: view[client.ProtectedOperation[State, Permission, i64]], grant: auth.Grant[Permission], input: i64) -> Result[client.Response, client.Failure] { return await client.execute(operation, grant, input); }\nfn main() { print(0); }\n",
    );
}

#[test]
fn explicit_public_operation_checks_without_grant_in_three_forms() {
    Fixture::new().three_forms(
        "import std.http.client as client\nclass State:\n    n: i64\nasync def call(operation: view[client.PublicOperation[State, i64]], input: i64) -> Result[client.Response, client.Failure]:\n    return await client.execute_public(operation, input)\ndef main():\n    print(0)\n",
        "import std.http.client as client\nrecord State { n: i64; }\nasync fn call(operation: view[client.PublicOperation[State, i64]], input: i64) -> Result[client.Response, client.Failure] { return await client.execute_public(operation, input); }\nfn main() { print(0); }\n",
    );
}

#[test]
fn response_move_while_body_view_lives_rejects_at_checker_in_both_forms() {
    let f = Fixture::new();
    f.rejected("main.nagi", "import std.http.client as client\ndef invalid(response: client.Response) -> bytes:\n    data = client.body(view(response))\n    result = client.take_body(response)\n    print(len(data))\n    return result\n", 4, "借用");
    f.rejected("manual.low", "import std.http.client as client\nfn invalid(response: client.Response) -> bytes {\n    let data = client.body(view(response));\n    let result = client.take_body(response);\n    print(len(data));\n    return result;\n}\n", 4, "借用");
}

#[test]
fn owned_response_wrapper_copy_is_rejected_but_shared_handle_copy_remains_valid() {
    let f = Fixture::new();
    f.rejected("main.nagi", "import std.http.client as client\nclass Envelope:\n    response: client.Response\ndef invalid(values: List[Envelope]) -> List[Envelope]:\n    return copy(view(values))\n", 5, "非Clone security resource");
    f.rejected("manual.low", "import std.http.client as client\nrecord Envelope { response: client.Response; }\nfn invalid(values: List[Envelope]) -> List[Envelope] {\n    return copy(view(values));\n}\n", 4, "非Clone security resource");
    f.three_forms("import std.http.client as client\ndef duplicate(values: List[shared[client.Response]]) -> List[shared[client.Response]]:\n    return copy(view(values))\ndef main():\n    print(0)\n", "import std.http.client as client\nfn duplicate(values: List[shared[client.Response]]) -> List[shared[client.Response]] { return copy(view(values)); }\nfn main() { print(0); }\n");
}

#[test]
fn protected_permission_mismatch_and_public_downgrade_are_checker_errors() {
    let f = Fixture::new();
    f.rejected("main.nagi", "import std.http.client as client\nimport std.auth as auth\nclass State:\n    n: i64\nclass Want:\n    n: i64\nclass Other:\n    n: i64\nasync def invalid(operation: view[client.ProtectedOperation[State, Want, i64]], grant: auth.Grant[Other]) -> Result[client.Response, client.Failure]:\n    return await client.execute(operation, grant, 1)\n", 10, "型が一致しません");
    f.rejected("manual.low", "import std.http.client as client\nimport std.auth as auth\nrecord State { n: i64; }\nrecord Want { n: i64; }\nrecord Other { n: i64; }\nasync fn invalid(operation: view[client.ProtectedOperation[State, Want, i64]], grant: auth.Grant[Other]) -> Result[client.Response, client.Failure] {\n    return await client.execute(operation, grant, 1);\n}\n", 7, "型が一致しません");
    f.rejected("main.nagi", "import std.http.client as client\nclass State:\n    n: i64\nclass Want:\n    n: i64\nasync def invalid(operation: view[client.ProtectedOperation[State, Want, i64]]) -> Result[client.Response, client.Failure]:\n    return await client.execute_public(operation, 1)\n", 7, "protected/public操作");
    f.rejected("manual.low", "import std.http.client as client\nrecord State { n: i64; }\nrecord Want { n: i64; }\nasync fn invalid(operation: view[client.ProtectedOperation[State, Want, i64]]) -> Result[client.Response, client.Failure] {\n    return await client.execute_public(operation, 1);\n}\n", 5, "protected/public操作");
}

#[test]
fn protocol_view_and_non_nominal_permission_are_rejected_at_declaration() {
    let f = Fixture::new();
    f.rejected("main.nagi", "import std.http.client as client\nclass State:\n    n: i64\ndef invalid(operation: view[client.PublicOperation[State, view[bytes]]]):\n    print(0)\n", 4, "SF06 input");
    f.rejected("manual.low", "import std.http.client as client\nrecord State { n: i64; }\nfn invalid(operation: view[client.PublicOperation[State, view[bytes]]]) {\n    print(0);\n}\n", 3, "SF06 input");
    f.rejected("main.nagi", "import std.http.client as client\nclass State:\n    n: i64\ndef invalid(operation: view[client.ProtectedOperation[State, i64, i64]]):\n    print(0)\n", 4, "Grantのpermission marker");
    f.rejected("manual.low", "import std.http.client as client\nrecord State { n: i64; }\nfn invalid(operation: view[client.ProtectedOperation[State, i64, i64]]) {\n    print(0);\n}\n", 3, "Grantのpermission marker");
}

#[test]
fn returned_header_views_do_not_borrow_lookup_name_in_three_forms() {
    Fixture::new().three_forms("import std.http.client as client\ndef read(response: view[client.Response]) -> Result[List[view[bytes]], Error]:\n    name = \"x-test\"\n    values = try client.headers(response, view(name))\n    name = \"different\"\n    return ok(values)\ndef main():\n    print(0)\n", "import std.http.client as client\nfn read(response: view[client.Response]) -> Result[List[view[bytes]], Error] {\n    let name = \"x-test\";\n    let values = try client.headers(response, view(name));\n    name = \"different\";\n    return ok(values);\n}\nfn main() { print(0); }\n");
}

#[test]
fn security_kind_cross_module_metadata_survives_saved_low_without_auth_alias() {
    Fixture::new().three_forms("import std.http.client as client\ndef code(failure: view[client.Failure]) -> i64:\n    match client.auth_kind(failure):\n        case Some(kind):\n            return 1\n        case None:\n            return 0\ndef main():\n    print(0)\n", "import std.http.client as client\nfn code(failure: view[client.Failure]) -> i64 {\n    match client.auth_kind(failure) {\n        case Some(kind) { return 1; }\n        case None { return 0; }\n    }\n}\nfn main() { print(0); }\n");
}

#[test]
fn actual_future_grant_capture_cannot_be_delegated_to_spawn() {
    let f = Fixture::new();
    let high = "import std.http.client as client\nimport std.auth as auth\nclass Config:\n    label: str\nenum Read:\n    Permission\nclass State:\n    operation: client.ProtectedOperation[Config, Read, bytes]\nasync def invoke(state: shared[State], grant: auth.Grant[Read], input: bytes) -> Result[client.Response, client.Failure]:\n    return await client.execute(view(state.operation), grant, input)\nasync def invalid(state: shared[State], grant: auth.Grant[Read], input: bytes) -> Result[unit, Error]:\n    async with scope:\n        task = spawn invoke(state, grant, input)\n        result = await task\n    return ok(print(0))\n";
    f.rejected("main.nagi", high, 13, "auth proof");
    let low = "import std.http.client as client\nimport std.auth as auth\nrecord Config { label: str; }\nenum Read { Permission; }\nrecord State { operation: client.ProtectedOperation[Config, Read, bytes]; }\nasync fn invoke(state: shared[State], grant: auth.Grant[Read], input: bytes) -> Result[client.Response, client.Failure] { return await client.execute(view(state.operation), grant, input); }\nasync fn invalid(state: shared[State], grant: auth.Grant[Read], input: bytes) -> Result[unit, Error] {\n    scope {\n        let task = spawn invoke(state, grant, input);\n        let result = await task;\n    }\n    return ok(print(0));\n}\n";
    f.rejected("manual.low", low, 9, "auth proof");
}

#[test]
fn only_context_payload_is_shared_and_permission_is_nominal() {
    let f = Fixture::new();
    f.three_forms("import std.http.client as client\nimport std.auth as auth\nclass Config:\n    label: str\nclass Marker:\n    failure: auth.Failure\ndef retain(operation: client.ProtectedOperation[Config, Marker, i64]) -> shared[client.ProtectedOperation[Config, Marker, i64]]:\n    return share(operation)\ndef main():\n    print(0)\n", "import std.http.client as client\nimport std.auth as auth\nrecord Config { label: str; }\nrecord Marker { failure: auth.Failure; }\nfn retain(operation: client.ProtectedOperation[Config, Marker, i64]) -> shared[client.ProtectedOperation[Config, Marker, i64]] { return share(operation); }\nfn main() { print(0); }\n");
    f.rejected("main.nagi", "import std.http.client as client\nimport std.auth as auth\nclass Marker:\n    n: i64\ndef invalid(operation: view[client.ProtectedOperation[auth.AuthScope, Marker, i64]]):\n    print(0)\n", 5, "auth proofを内部のshared state/context");
    f.rejected("manual.low", "import std.http.client as client\nimport std.auth as auth\nrecord Marker { n: i64; }\nfn invalid(operation: view[client.ProtectedOperation[auth.AuthScope, Marker, i64]]) {\n    print(0);\n}\n", 4, "auth proofを内部のshared state/context");
}
