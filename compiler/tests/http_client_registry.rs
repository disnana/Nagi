use nagic::stdlib::{self, Passing};
type PassingExpected = (&'static str, usize, &'static [Passing], Option<usize>, bool);

fn client_module() -> nagic::ast::ModuleId {
    stdlib::module("std.http.client").expect("adopted SF06 canonical module is not registered")
}

#[test]
fn adopted_client_resource_inventory_has_owned_nonclone_and_distinct_operations() {
    let module = client_module();
    let expected = [
        ("ProtectedOperation", 3, false, true),
        ("PublicOperation", 2, false, true),
        ("Response", 0, false, false),
        ("Failure", 0, false, true),
        ("FailureKind", 0, true, true),
        ("Outcome", 0, true, true),
    ];
    let actual: Vec<_> = stdlib::RESOURCES
        .iter()
        .copied()
        .filter(|r| stdlib::resource_module(*r) == module)
        .collect();
    assert_eq!(actual.len(), expected.len());
    for (name, arity, copy, debug) in expected {
        let r = stdlib::resource_named(&module, name).expect(name);
        let info = stdlib::resource_info(r);
        assert_eq!(
            (info.arity, info.copy, info.equality, info.debug),
            (arity, copy, copy, debug),
            "{name}"
        );
        assert!(info.storage && info.shared, "{name}");
        assert!(
            info.inline_type_arguments.is_empty(),
            "{name}: protocol is not inline payload"
        );
        assert!(
            stdlib::fields(r).is_empty(),
            "{name}: raw resource fields must remain private"
        );
    }
    let inbound = stdlib::module("std.http.server").unwrap();
    assert_ne!(
        stdlib::resource_named(&module, "Response"),
        stdlib::resource_named(&inbound, "Response")
    );
}

#[test]
fn adopted_client_passing_and_borrow_inventory() {
    let module = client_module();
    let expected: &[PassingExpected] = &[
        (
            "execute",
            3,
            &[Passing::Reference, Passing::Move, Passing::Move],
            None,
            true,
        ),
        (
            "execute_public",
            2,
            &[Passing::Reference, Passing::Move],
            None,
            true,
        ),
        ("status", 0, &[Passing::Reference], None, false),
        (
            "headers",
            0,
            &[Passing::Reference, Passing::Reference],
            Some(0),
            false,
        ),
        ("body", 0, &[Passing::Reference], Some(0), false),
        ("take_body", 0, &[Passing::Move], None, false),
        ("kind", 0, &[Passing::Reference], None, false),
        ("message", 0, &[Passing::Reference], Some(0), false),
        ("auth_kind", 0, &[Passing::Reference], None, false),
        ("outcome", 0, &[Passing::Reference], None, false),
    ];
    let operations: Vec<_> = stdlib::OPERATIONS
        .iter()
        .copied()
        .filter(|o| stdlib::operation_module(*o) == module)
        .collect();
    assert_eq!(operations.len(), expected.len());
    for (name, generics, passing, borrow, asynchronous) in expected {
        let info = operations
            .iter()
            .map(|o| stdlib::operation_info(*o))
            .find(|i| i.name == *name)
            .expect(name);
        assert_eq!(info.generic_arity, *generics, "{name}");
        assert_eq!(info.parameters, *passing, "{name}");
        assert_eq!(info.borrow_owner, *borrow, "{name}");
        assert_eq!(info.asynchronous, *asynchronous, "{name}");
        assert!(info.emit_type_arguments, "{name}");
    }
}

#[test]
fn adopted_client_failure_constants_keep_business_status_separate() {
    let module = client_module();
    let kind = stdlib::resource_named(&module, "FailureKind").unwrap();
    let outcome = stdlib::resource_named(&module, "Outcome").unwrap();
    let expected = [
        "INVALID_INPUT",
        "SECURITY",
        "DESTINATION_DENIED",
        "CAPACITY",
        "CLOSING",
        "DEADLINE",
        "DNS",
        "TCP",
        "TLS",
        "PROTOCOL",
        "RESPONSE_LIMIT",
        "UNSUPPORTED_ENCODING",
        "UNAVAILABLE",
        "INTERNAL",
    ];
    let actual: Vec<_> = stdlib::constants(kind).iter().map(|c| c.name).collect();
    assert_eq!(actual, expected);
    let outcomes: Vec<_> = stdlib::constants(outcome).iter().map(|c| c.name).collect();
    assert_eq!(outcomes, ["REQUEST_NOT_STARTED", "UNKNOWN"]);
}
