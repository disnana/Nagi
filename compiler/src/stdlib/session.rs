//! Canonical Session descriptors. Secret delivery is opaque and request-owned.
use super::*;
const fn plain(
    name: &'static str,
    path: &'static str,
    copy: bool,
    storage: bool,
    shared: bool,
    debug: bool,
) -> ResourceContract {
    ResourceContract::new(
        ResourceInfo {
            module: StandardModule::AuthSession,
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
static OPTIONS: ResourceContract = plain(
    "Options",
    "::nagi_runtime::auth::session::Options",
    false,
    true,
    true,
    true,
);
static COOKIE: ResourceContract = plain(
    "CookieOptions",
    "::nagi_runtime::auth::session::CookieOptions",
    false,
    true,
    true,
    true,
);
static STORE: ResourceContract = plain(
    "Store",
    "::nagi_runtime::auth::session::Store",
    false,
    true,
    true,
    true,
);
static FAILURE: ResourceContract = plain(
    "Failure",
    "::nagi_runtime::auth::session::Failure",
    false,
    true,
    false,
    true,
);
static RESPONSE: ResourceContract = plain(
    "SessionResponse",
    "::nagi_runtime::auth::session::SessionResponse",
    false,
    false,
    false,
    false,
)
.with_lifecycle(ResourceLifecycle::SameTask);
static SAME_SITE: ResourceContract = plain(
    "SameSite",
    "::nagi_runtime::auth::session::SameSite",
    true,
    true,
    true,
    true,
);
pub(super) fn resource_contract(resource: Resource) -> &'static ResourceContract {
    match resource {
        Resource::SessionOptions => &OPTIONS,
        Resource::SessionCookieOptions => &COOKIE,
        Resource::SessionStore => &STORE,
        Resource::SessionFailure => &FAILURE,
        Resource::SessionResponse => &RESPONSE,
        Resource::SessionSameSite => &SAME_SITE,
        _ => unreachable!("non-Session descriptor"),
    }
}
macro_rules! operation {
    ($name:literal,$path:literal,$passing:expr,$borrow:expr,$async:expr,$signature:literal) => {{
        static INFO: OperationInfo = OperationInfo {
            module: StandardModule::AuthSession,
            name: $name,
            rust_path: $path,
            arity: ($passing as &[Passing]).len(),
            generic_arity: 0,
            type_parameters: &[],
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
        Operation::SessionOptions => operation!("options","::nagi_runtime::auth::session::options",&[Move,Move,Move,Move,Move,Move,Move,Move],None,false,"(max_live: i64, max_stored: i64, max_row_bytes: i64, cleanup_batch: i64, idle_ms: i64, absolute_ms: i64, touch_ms: i64, collision_attempts: i64) -> Result[Options, Failure]"),
        Operation::SessionCookieOptions => operation!("cookie_options","::nagi_runtime::auth::session::cookie_options",&[Reference,Move],None,false,"(name: view[str], same_site: SameSite) -> Result[CookieOptions, Failure]"),
        Operation::SessionOpen => operation!("open","::nagi_runtime::auth::session::open",&[Reference,Move,Move],None,true,"(pool: view[Pool], options: Options, cookie: CookieOptions) -> Future[Result[Store, Failure]]"),
        Operation::SessionCloneStore => operation!("clone_store","::nagi_runtime::auth::session::clone_store",&[Reference],None,false,"(store: view[Store]) -> Store"),
        Operation::SessionIssue => operation!("issue","::nagi_runtime::auth::session::issue",&[Reference,Move],None,true,"(store: view[Store], scope: AuthScope) -> Future[Result[SessionResponse, Failure]]"),
        Operation::SessionRotate => operation!("rotate","::nagi_runtime::auth::session::rotate",&[Reference,Move],None,true,"(store: view[Store], scope: AuthScope) -> Future[Result[SessionResponse, Failure]]"),
        Operation::SessionLogout => operation!("logout","::nagi_runtime::auth::session::logout",&[Reference,Move],None,true,"(store: view[Store], scope: AuthScope) -> Future[Result[SessionResponse, Failure]]"),
        Operation::SessionApply => operation!("apply","::nagi_runtime::auth::session::apply",&[Move,Move],None,false,"(response: Response, intent: SessionResponse) -> Result[Response, Failure]"),
        Operation::SessionKind => operation!("kind","::nagi_runtime::auth::session::kind",&[Reference],None,false,"(failure: view[Failure]) -> FailureKind"),
        Operation::SessionMessage => operation!("message","::nagi_runtime::auth::session::message",&[Reference],Some(0),false,"(failure: view[Failure]) -> view[str]"),
        Operation::SessionOutcome => operation!("outcome","::nagi_runtime::auth::session::outcome",&[Reference],None,false,"(failure: view[Failure]) -> Outcome"),
        _ => unreachable!("non-Session operation"),
    }
}
pub(super) const SAME_SITE_CONSTANTS: &[ConstantInfo] = &[
    ConstantInfo {
        name: "STRICT",
        native_name: "Strict",
    },
    ConstantInfo {
        name: "LAX",
        native_name: "Lax",
    },
    ConstantInfo {
        name: "NONE",
        native_name: "None",
    },
];
