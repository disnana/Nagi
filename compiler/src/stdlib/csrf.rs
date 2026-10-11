//! Canonical B1 Session-CSRF data and operation descriptors. Runtime validates store identity.
use super::*;

const fn plain(name: &'static str, path: &'static str, shared: bool) -> ResourceContract {
    ResourceContract::new(
        ResourceInfo {
            module: StandardModule::AuthCsrf,
            name,
            arity: 0,
            type_parameters: &[],
            inline_type_arguments: &[],
            rust_path: path,
            copy: false,
            equality: false,
            storage: true,
            shared,
            debug: true,
        },
        &[],
        &[],
        &[],
        &[],
    )
}
static OPTIONS: ResourceContract = plain("Options", "::nagi_runtime::auth::csrf::Options", true);
static STORE: ResourceContract = plain("CsrfStore", "::nagi_runtime::auth::csrf::CsrfStore", true);
static FAILURE: ResourceContract = plain("Failure", "::nagi_runtime::auth::csrf::Failure", false);

pub(super) fn resource_contract(resource: Resource) -> &'static ResourceContract {
    match resource {
        Resource::CsrfOptions => &OPTIONS,
        Resource::CsrfStore => &STORE,
        Resource::CsrfFailure => &FAILURE,
        _ => unreachable!("non-CSRF descriptor"),
    }
}
macro_rules! operation {
    ($module:ident,$name:literal,$path:literal,$types:expr,$passing:expr,$borrow:expr,$async:expr,$signature:literal) => {{
        static INFO: OperationInfo = OperationInfo {
            module: StandardModule::$module,
            name: $name,
            rust_path: $path,
            arity: ($passing as &[Passing]).len(),
            generic_arity: ($types as &[&str]).len(),
            type_parameters: $types,
            asynchronous: $async,
            emit_type_arguments: false,
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
        Operation::CsrfOptions => operation!(AuthCsrf,"options","::nagi_runtime::auth::csrf::options",&[],&[Move,Move,Move,Move,Move],None,false,"(max_stored: i64, max_payload_bytes: i64, cleanup_batch: i64, ttl_ms: i64, collision_attempts: i64) -> Result[Options, Failure]"),
        Operation::CsrfOpen => operation!(AuthCsrf,"open","::nagi_runtime::auth::csrf::open",&[],&[Reference,Move,Reference],None,true,"(store: view[Store], options: Options, http: view[HttpOptions]) -> Future[Result[CsrfStore, Failure]]"),
        Operation::CsrfCloneStore => operation!(AuthCsrf,"clone_store","::nagi_runtime::auth::csrf::clone_store",&[],&[Reference],None,false,"(store: view[CsrfStore]) -> CsrfStore"),
        Operation::CsrfKind => operation!(AuthCsrf,"kind","::nagi_runtime::auth::csrf::kind",&[],&[Reference],None,false,"(failure: view[Failure]) -> FailureKind"),
        Operation::CsrfMessage => operation!(AuthCsrf,"message","::nagi_runtime::auth::csrf::message",&[],&[Reference],Some(0),false,"(failure: view[Failure]) -> view[str]"),
        Operation::CsrfOutcome => operation!(AuthCsrf,"outcome","::nagi_runtime::auth::csrf::outcome",&[],&[Reference],None,false,"(failure: view[Failure]) -> Outcome"),
        Operation::SessionCsrfTokens => operation!(HttpServer,"session_csrf_tokens","::nagi_runtime::http_server::session_csrf_tokens",&["S","E"],&[Move,Reference,Move],None,false,"[S, E](app: App[S, E], path: view[str], store: CsrfStore) -> Result[App[S, E], Error]"),
        Operation::SessionCsrfPolicy => operation!(HttpServer,"session_csrf_policy","::nagi_runtime::http_server::session_csrf_policy",&["S","A"],&[Move,Move],None,false,"[S, A](policy: Policy[S, A], store: CsrfStore) -> Result[Policy[S, A], Error]"),
        _ => unreachable!("non-CSRF operation"),
    }
}
