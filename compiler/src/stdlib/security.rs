//! Security Foundation canonical resources and operations.
use super::*;
const fn plain(
    module: StandardModule,
    name: &'static str,
    path: &'static str,
    copy: bool,
    storage: bool,
    shared: bool,
    debug: bool,
) -> ResourceContract {
    ResourceContract::new(
        ResourceInfo {
            module,
            name,
            arity: 0,
            type_parameters: &[],
            inline_type_arguments: &[],
            rust_path: path,
            copy,
            equality: copy,
            storage,
            shared,
            debug,
        },
        &[],
        &[],
        &[],
        &[],
    )
}
static SCOPE: ResourceContract = plain(
    StandardModule::Auth,
    "AuthScope",
    "::nagi_runtime::auth::AuthScope",
    false,
    false,
    false,
    false,
)
.with_lifecycle(ResourceLifecycle::SameTask);
static IDENTITY: ResourceContract = plain(
    StandardModule::Auth,
    "VerifiedIdentity",
    "::nagi_runtime::auth::VerifiedIdentity",
    false,
    false,
    false,
    false,
);
static FAILURE: ResourceContract = plain(
    StandardModule::Auth,
    "Failure",
    "::nagi_runtime::auth::Failure",
    false,
    true,
    false,
    true,
);
static KIND: ResourceContract = plain(
    StandardModule::Auth,
    "FailureKind",
    "::nagi_runtime::auth::FailureKind",
    true,
    true,
    true,
    true,
);
static POLICY: ResourceContract = ResourceContract::new(
    ResourceInfo {
        module: StandardModule::HttpServer,
        name: "Policy",
        arity: 2,
        type_parameters: &["S", "A"],
        inline_type_arguments: &[],
        rust_path: "::nagi_runtime::http_server::Policy",
        copy: false,
        equality: false,
        storage: true,
        shared: false,
        debug: true,
    },
    &[],
    &[0],
    &[1],
    &[],
);
pub(super) fn resource_contract(r: Resource) -> &'static ResourceContract {
    match r {
        Resource::AuthScope => &SCOPE,
        Resource::VerifiedIdentity => &IDENTITY,
        Resource::AuthFailure => &FAILURE,
        Resource::AuthFailureKind => &KIND,
        Resource::HttpPolicy => &POLICY,
        _ => unreachable!(),
    }
}
macro_rules! operation {
    ($module:ident,$name:literal,$path:literal,$types:expr,$emit:expr,$passing:expr,$borrow:expr,$signature:literal) => {{
        static INFO: OperationInfo = OperationInfo {
            module: StandardModule::$module,
            name: $name,
            rust_path: $path,
            arity: ($passing as &[Passing]).len(),
            generic_arity: ($types as &[&str]).len(),
            type_parameters: $types,
            asynchronous: false,
            emit_type_arguments: $emit,
            parameters: $passing,
            borrow_owner: $borrow,
            signature: $signature,
        };
        &INFO
    }};
}
pub(super) fn operation_info(op: Operation) -> &'static OperationInfo {
    use Passing::*;
    match op {
 Operation::PublicPolicy=>operation!(HttpServer,"public_policy","::nagi_runtime::http_server::public_policy",&["S"],true,&[],None,"[S]() -> Policy[S, unit]"),
 Operation::AuthenticatedPolicy=>operation!(HttpServer,"authenticated_policy","::nagi_runtime::http_server::authenticated_policy",&["S"],false,&[Handler],None,"[S](verifier: fn[Request, shared[S], Future[Result[VerifiedIdentity, Failure]]]) -> Policy[S, AuthScope]"),
 Operation::AuthorizedPolicy=>operation!(HttpServer,"authorized_policy","::nagi_runtime::http_server::authorized_policy",&["S","P"],false,&[Handler,Handler],None,"[S, P](verifier: fn[Request, shared[S], Future[Result[VerifiedIdentity, Failure]]], authorizer: fn[AuthScope, Request, shared[S], Future[Result[Grant[P], Failure]]]) -> Policy[S, Grant[P]]"),
 Operation::SecurityTimeout=>operation!(HttpServer,"security_timeout","::nagi_runtime::http_server::security_timeout",&[],true,&[Move,Move],None,"(options: Options, ms: i64) -> Result[Options, Error]"),
 Operation::AuthSubject=>operation!(Auth,"subject","::nagi_runtime::auth::subject",&[],true,&[Reference],None,"(scope: view[AuthScope]) -> i64"),
 Operation::AuthKind=>operation!(Auth,"kind","::nagi_runtime::auth::kind",&[],true,&[Reference],None,"(failure: view[Failure]) -> FailureKind"),
 Operation::AuthMessage=>operation!(Auth,"message","::nagi_runtime::auth::message",&[],true,&[Reference],Some(0),"(failure: view[Failure]) -> view[str]"),
 Operation::AuthInvalidCredential=>operation!(Auth,"invalid_credential","::nagi_runtime::auth::invalid_credential",&[],true,&[],None,"() -> Failure"),
 Operation::AuthDenied=>operation!(Auth,"denied","::nagi_runtime::auth::denied",&[],true,&[],None,"() -> Failure"),
 Operation::AuthExpired=>operation!(Auth,"expired","::nagi_runtime::auth::expired",&[],true,&[],None,"() -> Failure"),
 Operation::AuthInvalidRequest=>operation!(Auth,"invalid_request","::nagi_runtime::auth::invalid_request",&[],true,&[],None,"() -> Failure"),
 Operation::AuthUnavailable=>operation!(Auth,"unavailable","::nagi_runtime::auth::unavailable",&[],true,&[],None,"() -> Failure"),
 Operation::AuthInternal=>operation!(Auth,"internal","::nagi_runtime::auth::internal",&[],true,&[],None,"() -> Failure"),
 _=>unreachable!(),
 }
}
pub(super) const FAILURE_CONSTANTS: &[ConstantInfo] = &[
    ConstantInfo {
        name: "INVALID_CREDENTIAL",
        native_name: "INVALID_CREDENTIAL",
    },
    ConstantInfo {
        name: "DENIED",
        native_name: "DENIED",
    },
    ConstantInfo {
        name: "EXPIRED",
        native_name: "EXPIRED",
    },
    ConstantInfo {
        name: "INVALID_REQUEST",
        native_name: "INVALID_REQUEST",
    },
    ConstantInfo {
        name: "UNAVAILABLE",
        native_name: "UNAVAILABLE",
    },
    ConstantInfo {
        name: "INTERNAL",
        native_name: "INTERNAL",
    },
];
