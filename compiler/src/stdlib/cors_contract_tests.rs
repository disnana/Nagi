//! Independent CORS descriptors; source-only declarations do not prove native execution.
use super::*;
#[test]
fn cors_resources_roles_capabilities_and_constants_are_canonical() {
    let module = ModuleId("stdlib:std.http.server".into());
    for (resource, name, capabilities) in [
        (
            Resource::CorsPolicy,
            "CorsPolicy",
            [false, false, true, true, true],
        ),
        (
            Resource::CorsCredentials,
            "CorsCredentials",
            [true, true, true, true, true],
        ),
    ] {
        let info = resource_info(resource);
        assert_eq!(info.module, StandardModule::HttpServer);
        assert_eq!(info.name, name);
        assert_eq!(
            info.rust_path,
            format!("::nagi_runtime::http_server::{name}")
        );
        assert_eq!(
            [
                info.copy,
                info.equality,
                info.storage,
                info.shared,
                info.debug
            ],
            capabilities
        );
        assert_eq!(info.arity, 0);
        assert!(info.type_parameters.is_empty() && info.inline_type_arguments.is_empty());
        assert_eq!(resource_named(&module, name), Some(resource));
        assert!(!native_serde_supported(resource) && !requires_same_task(resource));
        assert!(fields(resource).is_empty());
    }
    assert_eq!(
        constants(Resource::CorsCredentials)
            .iter()
            .map(|v| v.name)
            .collect::<Vec<_>>(),
        vec!["OMIT", "ALLOW"]
    );
    assert!(constants(Resource::CorsPolicy).is_empty());
    assert!(crate::capabilities::contains_security_nonclone(
        &resource_type(Resource::CorsPolicy, vec![]),
        &Default::default(),
        &Default::default()
    ));
    assert!(!crate::capabilities::contains_security_nonshared(
        &resource_type(Resource::CorsPolicy, vec![]),
        &Default::default(),
        &Default::default()
    ));
}
#[test]
fn cors_operations_keep_owned_config_borrowed_pattern_and_app_generic_roles() {
    for (op, name, parameters, generics) in [
        (
            Operation::CorsPolicy,
            "cors_policy",
            vec![Passing::Move; 8],
            vec![],
        ),
        (
            Operation::CorsRoute,
            "cors_route",
            vec![
                Passing::Move,
                Passing::Move,
                Passing::Reference,
                Passing::Move,
            ],
            vec!["S", "E"],
        ),
    ] {
        let info = operation_info(op);
        assert_eq!(info.name, name);
        assert_eq!(
            info.rust_path,
            format!("::nagi_runtime::http_server::{name}")
        );
        assert_eq!(info.module, StandardModule::HttpServer);
        assert_eq!(info.parameters, parameters);
        assert_eq!(info.arity, parameters.len());
        assert_eq!(info.type_parameters, generics);
        assert_eq!(info.generic_arity, generics.len());
        assert!(!info.asynchronous && !info.emit_type_arguments);
        assert_eq!(info.borrow_owner, None);
        assert_eq!(
            operation_semantics(op).emission,
            OperationEmission::NativeCall
        );
    }
    assert_eq!(
        resource_info(Resource::App).inline_type_arguments,
        &[] as &[usize]
    );
    assert_eq!(
        type_argument_positions(Resource::App, TypeArgumentRole::SharedPayload),
        &[0]
    );
    assert_eq!(
        type_argument_positions(Resource::App, TypeArgumentRole::CallbackSignature),
        &[1]
    );
}
