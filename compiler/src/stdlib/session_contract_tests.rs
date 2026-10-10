//! Independent adopted Session ownership/borrow/serde expectations, not descriptor-derived.
use super::*;
#[test]
fn session_resource_contracts_and_closed_constants_are_canonical() {
    assert_eq!(
        module("std.auth.session"),
        Some(ModuleId("stdlib:std.auth.session".into()))
    );
    for (r, name, flags, same_task) in [
        (
            Resource::SessionOptions,
            "Options",
            [false, false, true, true, true],
            false,
        ),
        (
            Resource::SessionCookieOptions,
            "CookieOptions",
            [false, false, true, true, true],
            false,
        ),
        (
            Resource::SessionStore,
            "Store",
            [false, false, true, true, true],
            false,
        ),
        (
            Resource::SessionFailure,
            "Failure",
            [false, false, true, false, true],
            false,
        ),
        (
            Resource::SessionResponse,
            "SessionResponse",
            [false, false, false, false, false],
            true,
        ),
        (
            Resource::SessionSameSite,
            "SameSite",
            [true, true, true, true, true],
            false,
        ),
    ] {
        let info = resource_info(r);
        assert_eq!(info.name, name);
        assert_eq!(info.module, StandardModule::AuthSession);
        assert_eq!(
            resource_module(r),
            ModuleId("stdlib:std.auth.session".into())
        );
        assert_eq!(
            info.rust_path,
            format!("::nagi_runtime::auth::session::{name}")
        );
        assert_eq!(
            [
                info.copy,
                info.equality,
                info.storage,
                info.shared,
                info.debug
            ],
            flags
        );
        assert_eq!(info.arity, 0);
        assert!(info.type_parameters.is_empty() && info.inline_type_arguments.is_empty());
        assert!(fields(r).is_empty());
        assert!(!native_serde_supported(r));
        assert_eq!(requires_same_task(r), same_task);
        for role in [
            TypeArgumentRole::InlinePayload,
            TypeArgumentRole::SharedPayload,
            TypeArgumentRole::IndirectProtocol,
            TypeArgumentRole::CallbackSignature,
            TypeArgumentRole::NominalPhantom,
        ] {
            assert!(type_argument_positions(r, role).is_empty());
        }
    }
    assert_eq!(
        constants(Resource::SessionSameSite)
            .iter()
            .map(|c| (c.name, c.native_name))
            .collect::<Vec<_>>(),
        vec![("STRICT", "Strict"), ("LAX", "Lax"), ("NONE", "None")]
    );
    assert_ne!(
        resource_id(Resource::SessionFailure),
        resource_id(Resource::AuthFailure)
    );
    assert_ne!(
        resource_id(Resource::SessionOptions),
        resource_id(Resource::Options)
    );
}
#[test]
fn session_call_passing_and_message_owner_are_fixed() {
    use Passing::{Handler as H, Move as M, Reference as R};
    for (op, name, passing, asynchronous, borrowed) in [
        (
            Operation::SessionOptions,
            "options",
            vec![M, M, M, M, M, M, M, M],
            false,
            None,
        ),
        (
            Operation::SessionCookieOptions,
            "cookie_options",
            vec![R, M],
            false,
            None,
        ),
        (Operation::SessionOpen, "open", vec![R, M, M], true, None),
        (
            Operation::SessionCloneStore,
            "clone_store",
            vec![R],
            false,
            None,
        ),
        (Operation::SessionIssue, "issue", vec![R, M], true, None),
        (Operation::SessionRotate, "rotate", vec![R, M], true, None),
        (Operation::SessionLogout, "logout", vec![R, M], true, None),
        (Operation::SessionApply, "apply", vec![M, M], false, None),
        (Operation::SessionKind, "kind", vec![R], false, None),
        (
            Operation::SessionMessage,
            "message",
            vec![R],
            false,
            Some(0),
        ),
        (Operation::SessionOutcome, "outcome", vec![R], false, None),
    ] {
        let info = operation_info(op);
        assert_eq!(info.name, name);
        assert_eq!(info.module, StandardModule::AuthSession);
        assert_eq!(
            info.rust_path,
            format!("::nagi_runtime::auth::session::{name}")
        );
        assert_eq!(info.parameters, passing);
        assert_eq!(info.arity, passing.len());
        assert_eq!(info.asynchronous, asynchronous);
        assert_eq!(info.borrow_owner, borrowed);
        assert_eq!(info.generic_arity, 0);
        assert!(!info.emit_type_arguments);
    }
    for (op, passing, generics, emitted) in [
        (
            Operation::SessionAuthenticatedPolicy,
            vec![M],
            vec!["S"],
            true,
        ),
        (
            Operation::SessionAuthorizedPolicy,
            vec![M, H],
            vec!["S", "P"],
            false,
        ),
    ] {
        let info = operation_info(op);
        assert_eq!(info.module, StandardModule::HttpServer);
        assert_eq!(info.parameters, passing);
        assert_eq!(info.type_parameters, generics);
        assert_eq!(info.emit_type_arguments, emitted);
        assert!(!info.asynchronous && info.borrow_owner.is_none());
    }
}
#[test]
fn session_payload_capabilities_follow_the_existing_nominal_walk() {
    let classes = std::collections::HashMap::new();
    let enums = std::collections::HashMap::new();
    let intent = resource_type(Resource::SessionResponse, vec![]);
    for t in [
        intent.clone(),
        Type::generic("Option", vec![intent.clone()]),
        Type::generic("Result", vec![intent, Type::named("i64")]),
    ] {
        assert!(crate::capabilities::contains_auth_proof(
            &t, &classes, &enums
        ));
        assert!(crate::capabilities::contains_security_nonclone(
            &t, &classes, &enums
        ));
        assert!(crate::capabilities::contains_same_task_resource(
            &t, &classes, &enums
        ));
        assert!(!crate::capabilities::debug_supported(&t, &classes, &enums));
    }
    let store = resource_type(Resource::SessionStore, vec![]);
    assert!(crate::capabilities::contains_security_nonclone(
        &store, &classes, &enums
    ));
    assert!(!crate::capabilities::contains_security_nonshared(
        &store, &classes, &enums
    ));
    assert!(!crate::capabilities::contains_auth_proof(
        &store, &classes, &enums
    ));
    assert!(crate::capabilities::debug_supported(
        &store, &classes, &enums
    ));
    let shared = Type::generic("shared", vec![store]);
    assert!(!crate::capabilities::contains_security_nonclone(
        &shared, &classes, &enums
    ));
    assert!(!crate::capabilities::contains_same_task_resource(
        &shared, &classes, &enums
    ));
}
