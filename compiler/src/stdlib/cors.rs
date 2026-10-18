//! Opaque finite CORS configuration; no authorization proof or lifecycle role.
use super::*;
const fn plain(name: &'static str, path: &'static str, copy: bool) -> ResourceContract {
    ResourceContract::new(
        ResourceInfo {
            module: StandardModule::HttpServer,
            name,
            arity: 0,
            type_parameters: &[],
            inline_type_arguments: &[],
            rust_path: path,
            copy,
            equality: copy,
            storage: true,
            shared: true,
            debug: true,
        },
        &[],
        &[],
        &[],
        &[],
    )
}
static POLICY: ResourceContract = plain(
    "CorsPolicy",
    "::nagi_runtime::http_server::CorsPolicy",
    false,
);
static CREDENTIALS: ResourceContract = plain(
    "CorsCredentials",
    "::nagi_runtime::http_server::CorsCredentials",
    true,
);
pub(super) fn resource_contract(resource: Resource) -> &'static ResourceContract {
    match resource {
        Resource::CorsPolicy => &POLICY,
        Resource::CorsCredentials => &CREDENTIALS,
        _ => unreachable!("non CORS resource"),
    }
}
pub(super) fn operation_info(operation: Operation) -> &'static OperationInfo {
    static POLICY: OperationInfo = OperationInfo {
        module: StandardModule::HttpServer, name: "cors_policy", rust_path: "::nagi_runtime::http_server::cors_policy",
        arity: 8, generic_arity: 0, type_parameters: &[], asynchronous: false, emit_type_arguments: false,
        parameters: &[Passing::Move; 8], borrow_owner: None,
        signature: "(origins: List[str], request_headers: List[str], credentials: CorsCredentials, max_age_seconds: i64, max_origins: i64, max_origin_bytes: i64, max_headers: i64, max_header_bytes: i64) -> Result[CorsPolicy, Error]",
    };
    static ROUTE: OperationInfo = OperationInfo {
        module: StandardModule::HttpServer, name: "cors_route", rust_path: "::nagi_runtime::http_server::cors_route",
        arity: 4, generic_arity: 2, type_parameters: &["S", "E"], asynchronous: false, emit_type_arguments: false,
        parameters: &[Passing::Move, Passing::Move, Passing::Reference, Passing::Move], borrow_owner: None,
        signature: "[S, E](app: App[S, E], method: Method, registered_path: view[str], cors: CorsPolicy) -> Result[App[S, E], Error]",
    };
    match operation {
        Operation::CorsPolicy => &POLICY,
        Operation::CorsRoute => &ROUTE,
        _ => unreachable!("non CORS operation"),
    }
}
pub(super) const CREDENTIAL_CONSTANTS: &[ConstantInfo] = &[
    ConstantInfo {
        name: "OMIT",
        native_name: "OMIT",
    },
    ConstantInfo {
        name: "ALLOW",
        native_name: "ALLOW",
    },
];
