//! Independent B1 compiler contract oracle, including runtime-only store identity boundary.
use super::*;
#[test]
fn csrf_module_resources_are_nominal_opaque_nonclone_data() {
    let id = ModuleId("stdlib:std.auth.csrf".into());
    assert_eq!(module("std.auth.csrf"), Some(id.clone()));
    for (resource, name, shared) in [
        (Resource::CsrfOptions, "Options", true),
        (Resource::CsrfStore, "CsrfStore", true),
        (Resource::CsrfFailure, "Failure", false),
    ] {
        let info = resource_info(resource);
        assert_eq!(info.module, StandardModule::AuthCsrf);
        assert_eq!(info.name, name);
        assert_eq!(
            info.rust_path,
            format!("::nagi_runtime::auth::csrf::{name}")
        );
        assert_eq!(
            [
                info.copy,
                info.equality,
                info.storage,
                info.shared,
                info.debug
            ],
            [false, false, true, shared, true]
        );
        assert_eq!(resource_module(resource), id);
        assert_eq!(resource_named(&id, name), Some(resource));
        assert!(fields(resource).is_empty() && constants(resource).is_empty());
        assert!(!native_serde_supported(resource) && !requires_same_task(resource));
        assert_ne!(resource_id(resource), resource_id(Resource::SessionStore));
    }
    let classes = std::collections::HashMap::new();
    let enums = std::collections::HashMap::new();
    for r in [
        Resource::CsrfOptions,
        Resource::CsrfStore,
        Resource::CsrfFailure,
    ] {
        assert!(crate::capabilities::contains_security_nonclone(
            &resource_type(r, vec![]),
            &classes,
            &enums
        ));
    }
    assert!(!crate::capabilities::contains_security_nonshared(
        &resource_type(Resource::CsrfStore, vec![]),
        &classes,
        &enums
    ));
    assert!(crate::capabilities::contains_security_nonshared(
        &resource_type(Resource::CsrfFailure, vec![]),
        &classes,
        &enums
    ));
}
#[test]
fn csrf_operations_keep_move_borrow_async_and_generic_shapes() {
    use Passing::{Move as M, Reference as R};
    for (op, name, module, passing, asynchronous, borrow, generic) in [
        (
            Operation::CsrfOptions,
            "options",
            StandardModule::AuthCsrf,
            vec![M, M, M, M, M],
            false,
            None,
            vec![],
        ),
        (
            Operation::CsrfOpen,
            "open",
            StandardModule::AuthCsrf,
            vec![R, M, R],
            true,
            None,
            vec![],
        ),
        (
            Operation::CsrfCloneStore,
            "clone_store",
            StandardModule::AuthCsrf,
            vec![R],
            false,
            None,
            vec![],
        ),
        (
            Operation::CsrfKind,
            "kind",
            StandardModule::AuthCsrf,
            vec![R],
            false,
            None,
            vec![],
        ),
        (
            Operation::CsrfMessage,
            "message",
            StandardModule::AuthCsrf,
            vec![R],
            false,
            Some(0),
            vec![],
        ),
        (
            Operation::CsrfOutcome,
            "outcome",
            StandardModule::AuthCsrf,
            vec![R],
            false,
            None,
            vec![],
        ),
        (
            Operation::SessionCsrfTokens,
            "session_csrf_tokens",
            StandardModule::HttpServer,
            vec![M, R, M],
            false,
            None,
            vec!["S", "E"],
        ),
        (
            Operation::SessionCsrfPolicy,
            "session_csrf_policy",
            StandardModule::HttpServer,
            vec![M, M],
            false,
            None,
            vec!["S", "A"],
        ),
    ] {
        let info = operation_info(op);
        assert_eq!(info.module, module);
        assert_eq!(info.name, name);
        assert_eq!(
            info.rust_path,
            format!("{}::{name}", module_info(module).rust_namespace)
        );
        assert_eq!(info.parameters, passing);
        assert_eq!(info.arity, passing.len());
        assert_eq!(info.asynchronous, asynchronous);
        assert_eq!(info.borrow_owner, borrow);
        assert_eq!(info.type_parameters, generic);
        assert_eq!(info.generic_arity, generic.len());
        assert!(!info.emit_type_arguments);
        assert_eq!(
            operation_semantics(op).emission,
            OperationEmission::NativeCall
        );
    }
}
