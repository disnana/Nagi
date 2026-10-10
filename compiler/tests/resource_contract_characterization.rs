//! Independent, hand-reviewed pre-refactor inventory. Expectations are not regenerated
//! from the registry. This characterizes legacy behavior, not a new resource policy.
use nagic::ast::{ModuleId, Type};
use nagic::stdlib::{self, Operation as O, Passing, Resource as R, StandardModule as M};
use std::collections::HashSet;

// name, module, generic labels, Copy/Eq/storage/shared/Debug, inline indexes.
type ResourceExpected = (
    R,
    M,
    &'static str,
    &'static [&'static str],
    [bool; 5],
    &'static [usize],
);
const RESOURCES: &[ResourceExpected] = &[
    (
        R::SqlitePool,
        M::Sqlite,
        "Pool",
        &[],
        [false, false, true, true, true],
        &[],
    ),
    (
        R::SqliteTx,
        M::Sqlite,
        "Tx",
        &[],
        [false, false, false, false, false],
        &[],
    ),
    (
        R::SqliteQueryValue,
        M::Sqlite,
        "Query",
        &[],
        [true, false, true, true, true],
        &[],
    ),
    (
        R::SqliteParameters,
        M::Sqlite,
        "Parameters",
        &[],
        [false, false, true, false, false],
        &[],
    ),
    (
        R::SqliteOptions,
        M::Sqlite,
        "Options",
        &[],
        [false, false, true, false, true],
        &[],
    ),
    (
        R::SqliteBeginMode,
        M::Sqlite,
        "BeginMode",
        &[],
        [true, true, true, true, true],
        &[],
    ),
    (
        R::SqliteFailure,
        M::Sqlite,
        "Failure",
        &[],
        [false, false, true, true, true],
        &[],
    ),
    (
        R::SqliteFailureKind,
        M::Sqlite,
        "FailureKind",
        &[],
        [true, true, true, true, true],
        &[],
    ),
    (
        R::SqliteOutcome,
        M::Sqlite,
        "Outcome",
        &[],
        [true, true, true, true, true],
        &[],
    ),
    (
        R::Task,
        M::Task,
        "Task",
        &["T"],
        [false, false, false, false, false],
        &[],
    ),
    (
        R::TaskFailure,
        M::Task,
        "TaskFailure",
        &[],
        [false, false, true, false, true],
        &[],
    ),
    (
        R::TaskFailureKind,
        M::Task,
        "TaskFailureKind",
        &[],
        [true, true, true, true, true],
        &[],
    ),
    (
        R::Request,
        M::HttpServer,
        "Request",
        &[],
        [false, false, true, true, true],
        &[],
    ),
    (
        R::Response,
        M::HttpServer,
        "Response",
        &[],
        [false, false, true, false, true],
        &[],
    ),
    (
        R::Method,
        M::HttpServer,
        "Method",
        &[],
        [false, true, true, true, true],
        &[],
    ),
    (
        R::Status,
        M::HttpServer,
        "Status",
        &[],
        [true, true, true, true, true],
        &[],
    ),
    (
        R::Options,
        M::HttpServer,
        "Options",
        &[],
        [false, false, true, true, true],
        &[],
    ),
    (
        R::App,
        M::HttpServer,
        "App",
        &["S", "E"],
        [false, false, true, false, true],
        &[],
    ),
    (
        R::Supervisor,
        M::Actor,
        "Supervisor",
        &["C"],
        [false, false, true, false, true],
        &[],
    ),
    (
        R::Control,
        M::Actor,
        "Control",
        &[],
        [false, false, true, true, true],
        &[],
    ),
    (
        R::Actor,
        M::Actor,
        "Actor",
        &["M", "R", "E"],
        [false, false, true, true, true],
        &[],
    ),
    (
        R::Turn,
        M::Actor,
        "Turn",
        &["S", "R", "E"],
        [false, false, true, false, true],
        &[0, 1, 2],
    ),
    (
        R::SupervisorOptions,
        M::Actor,
        "Options",
        &[],
        [false, false, true, true, true],
        &[],
    ),
    (
        R::ActorOptions,
        M::Actor,
        "ActorOptions",
        &[],
        [false, false, true, true, true],
        &[],
    ),
    (
        R::RestartPolicy,
        M::Actor,
        "RestartPolicy",
        &[],
        [true, true, true, true, true],
        &[],
    ),
    (
        R::CallKind,
        M::Actor,
        "CallKind",
        &[],
        [true, true, true, true, true],
        &[],
    ),
    (
        R::EventKind,
        M::Actor,
        "EventKind",
        &[],
        [true, true, true, true, true],
        &[],
    ),
    (
        R::CallError,
        M::Actor,
        "CallError",
        &[],
        [false, false, true, true, true],
        &[],
    ),
    (
        R::Event,
        M::Actor,
        "Event",
        &[],
        [false, false, true, true, true],
        &[],
    ),
    (
        R::TaskReady,
        M::Actor,
        "TaskReady",
        &[],
        [false, false, true, false, true],
        &[],
    ),
    (
        R::WaitKind,
        M::Actor,
        "WaitKind",
        &[],
        [true, true, true, true, true],
        &[],
    ),
    (
        R::WaitError,
        M::Actor,
        "WaitError",
        &[],
        [false, false, true, true, true],
        &[],
    ),
    (
        R::Principal,
        M::Auth,
        "Principal",
        &[],
        [false, false, false, false, false],
        &[],
    ),
    (
        R::Grant,
        M::Auth,
        "Grant",
        &["P"],
        [false, false, false, false, false],
        &[],
    ),
    (
        R::AuthScope,
        M::Auth,
        "AuthScope",
        &[],
        [false, false, false, false, false],
        &[],
    ),
    (
        R::VerifiedIdentity,
        M::Auth,
        "VerifiedIdentity",
        &[],
        [false, false, false, false, false],
        &[],
    ),
    (
        R::AuthFailure,
        M::Auth,
        "Failure",
        &[],
        [false, false, true, false, true],
        &[],
    ),
    (
        R::AuthFailureKind,
        M::Auth,
        "FailureKind",
        &[],
        [true, true, true, true, true],
        &[],
    ),
    (
        R::HttpPolicy,
        M::HttpServer,
        "Policy",
        &["S", "A"],
        [false, false, true, false, true],
        &[],
    ),
];
fn module(module: M) -> (&'static str, &'static str, &'static str) {
    match module {
        M::HttpServer => (
            "std.http.server",
            "stdlib:std.http.server",
            "::nagi_runtime::http_server",
        ),
        M::Actor => ("std.actor", "stdlib:std.actor", "::nagi_runtime::actor"),
        M::Result => ("std.result", "stdlib:std.result", "::nagi_runtime::result"),
        M::Auth => ("std.auth", "stdlib:std.auth", "::nagi_runtime::auth"),
        M::Ownership => ("std.ownership", "stdlib:std.ownership", "::std::convert"),
        M::Task => ("std.task", "stdlib:std.task", "::nagi_runtime"),
        M::Sqlite => (
            "std.db.sqlite",
            "stdlib:std.db.sqlite",
            "::nagi_runtime::sqlite",
        ),
    }
}
#[test]
fn registered_resources_match_the_independent_inventory() {
    let _: fn(R) -> &'static stdlib::ResourceInfo = stdlib::resource_info;
    // The public field names/types and struct-literal shape are compatibility
    // requirements even if storage is later backed by a private contract.
    let _public_literal = stdlib::ResourceInfo {
        module: M::Auth,
        name: "Example",
        arity: 0,
        type_parameters: &[],
        inline_type_arguments: &[],
        rust_path: "native::Example",
        copy: false,
        equality: false,
        storage: false,
        shared: false,
        debug: false,
    };
    assert_eq!(RESOURCES.len(), 39);
    assert_eq!(stdlib::RESOURCES.len(), 39);
    assert_eq!(
        stdlib::RESOURCES.iter().copied().collect::<HashSet<_>>(),
        RESOURCES.iter().map(|r| r.0).collect()
    );
    let mut identities = HashSet::new();
    for &(resource, owner, name, args, flags, inline) in RESOURCES {
        let info = stdlib::resource_info(resource);
        let (module_name, id, native) = module(owner);
        assert_eq!(info.module, owner, "{resource:?}");
        assert_eq!(info.name, name);
        assert_eq!(info.arity, args.len());
        assert_eq!(info.type_parameters, args);
        assert_eq!(
            [
                info.copy,
                info.equality,
                info.storage,
                info.shared,
                info.debug
            ],
            flags,
            "{resource:?}"
        );
        assert_eq!(info.inline_type_arguments, inline, "{resource:?}");
        assert!(inline.iter().all(|&i| i < args.len()));
        assert_eq!(
            inline.iter().copied().collect::<HashSet<_>>().len(),
            inline.len()
        );
        assert_eq!(info.rust_path, format!("{native}::{name}"));
        assert_eq!(stdlib::resource_module(resource), ModuleId(id.into()));
        assert_eq!(
            stdlib::resource_named(&ModuleId(id.into()), name),
            Some(resource)
        );
        assert!(identities.insert((id, name)));
        assert_eq!(stdlib::module_info(owner).name, module_name);
        assert_eq!(stdlib::module_info(owner).id, id);
        assert_eq!(stdlib::module_info(owner).rust_namespace, native);
    }
    assert_ne!(
        stdlib::resource_id(R::Options),
        stdlib::resource_id(R::SupervisorOptions)
    );
}

// Value arity is independently written; Passing never comes from generic labels.
// Passing letters: M=Move, B=Borrow, R=Reference, H=Handler, P=Mapper.
struct OperationExpected {
    operation: O,
    module: M,
    name: &'static str,
    arity: usize,
    generics: &'static [&'static str],
    passing: &'static str,
    borrow: Option<usize>,
    asynchronous: bool,
    emit_generics: bool,
    signature: &'static str,
}
macro_rules! op {
    ($op:ident, $module:ident, $name:literal, $arity:literal, $args:expr, $passing:literal,
     $borrow:expr, $async:literal, $emit:literal, $signature:literal) => {
        OperationExpected {
            operation: O::$op,
            module: M::$module,
            name: $name,
            arity: $arity,
            generics: $args,
            passing: $passing,
            borrow: $borrow,
            asynchronous: $async,
            emit_generics: $emit,
            signature: $signature,
        }
    };
}
const OPERATIONS: &[OperationExpected] = &[
    op!(PublicPolicy,HttpServer,"public_policy",0,&["S"],"",None,false,true,"[S]() -> Policy[S, unit]"),
    op!(AuthenticatedPolicy,HttpServer,"authenticated_policy",1,&["S"],"H",None,false,false,"[S](verifier: fn[Request, shared[S], Future[Result[VerifiedIdentity, Failure]]]) -> Policy[S, AuthScope]"),
    op!(AuthorizedPolicy,HttpServer,"authorized_policy",2,&["S","P"],"HH",None,false,false,"[S, P](verifier: fn[Request, shared[S], Future[Result[VerifiedIdentity, Failure]]], authorizer: fn[AuthScope, Request, shared[S], Future[Result[Grant[P], Failure]]]) -> Policy[S, Grant[P]]"),
    op!(SecurityTimeout,HttpServer,"security_timeout",2,&[],"MM",None,false,true,"(options: Options, ms: i64) -> Result[Options, Error]"),
    op!(AuthSubject,Auth,"subject",1,&[],"R",None,false,true,"(scope: view[AuthScope]) -> i64"),
    op!(AuthKind,Auth,"kind",1,&[],"R",None,false,true,"(failure: view[Failure]) -> FailureKind"),
    op!(AuthMessage,Auth,"message",1,&[],"R",Some(0),false,true,"(failure: view[Failure]) -> view[str]"),
    op!(AuthInvalidCredential,Auth,"invalid_credential",0,&[],"",None,false,true,"() -> Failure"),
    op!(AuthDenied,Auth,"denied",0,&[],"",None,false,true,"() -> Failure"),
    op!(AuthExpired,Auth,"expired",0,&[],"",None,false,true,"() -> Failure"),
    op!(AuthInvalidRequest,Auth,"invalid_request",0,&[],"",None,false,true,"() -> Failure"),
    op!(AuthUnavailable,Auth,"unavailable",0,&[],"",None,false,true,"() -> Failure"),
    op!(AuthInternal,Auth,"internal",0,&[],"",None,false,true,"() -> Failure"),

    op!(SqliteLiteral,Sqlite,"literal",1,&[],"M",None,false,false,"(sql: str) -> Query"),
    op!(SqliteOptions,Sqlite,"options",4,&[],"MMMM",None,false,false,"(connections: i64, queue_capacity: i64, acquire_ms: i64, busy_ms: i64) -> Result[Options, Error]"),
    op!(SqliteOpen,Sqlite,"open",2,&[],"RM",None,true,false,"(path: view[str], options: Options) -> Future[Result[Pool, Failure]]"),
    op!(SqliteClonePool,Sqlite,"clone_pool",1,&[],"R",None,false,false,"(pool: view[Pool]) -> Pool"),
    op!(SqliteBegin,Sqlite,"begin",2,&[],"RM",None,true,false,"(pool: view[Pool], mode: BeginMode) -> Future[Result[Tx, Failure]]"),
    op!(SqliteParameters,Sqlite,"parameters",0,&[],"",None,false,false,"() -> Parameters"),
    op!(SqliteBindI64,Sqlite,"bind_i64",2,&[],"MM",None,false,false,"(parameters: Parameters, value: i64) -> Parameters"),
    op!(SqliteBindF64,Sqlite,"bind_f64",2,&[],"MM",None,false,false,"(parameters: Parameters, value: f64) -> Result[Parameters, Error]"),
    op!(SqliteBindText,Sqlite,"bind_text",2,&[],"MM",None,false,false,"(parameters: Parameters, value: str) -> Parameters"),
    op!(SqliteBindBytes,Sqlite,"bind_bytes",2,&[],"MM",None,false,false,"(parameters: Parameters, value: bytes) -> Parameters"),
    op!(SqliteBindNull,Sqlite,"bind_null",1,&[],"M",None,false,false,"(parameters: Parameters) -> Parameters"),
    op!(SqliteQuery,Sqlite,"query",3,&["T"],"RMM",None,true,true,"[T](tx: view[Tx], query: Query, parameters: Parameters) -> Future[Result[Option[T], Failure]]"),
    op!(SqliteAll,Sqlite,"all",3,&["T"],"RMM",None,true,true,"[T](tx: view[Tx], query: Query, parameters: Parameters) -> Future[Result[List[T], Failure]]"),
    op!(SqliteExec,Sqlite,"exec",3,&[],"RMM",None,true,false,"(tx: view[Tx], query: Query, parameters: Parameters) -> Future[Result[i64, Failure]]"),
    op!(SqliteCommit,Sqlite,"commit",1,&[],"M",None,true,false,"(tx: Tx) -> Future[Result[unit, Failure]]"),
    op!(SqliteRollback,Sqlite,"rollback",1,&[],"M",None,true,false,"(tx: Tx) -> Future[Result[unit, Failure]]"),
    op!(SqliteClose,Sqlite,"close",2,&[],"RM",None,true,false,"(pool: view[Pool], timeout_ms: i64) -> Future[Result[unit, Failure]]"),
    op!(SqliteCopyPrimaryError,Sqlite,"copy_primary_error",1,&[],"R",None,false,false,"(problem: view[Failure]) -> Option[Error]"),
    op!(SqliteCopyCleanupError,Sqlite,"copy_cleanup_error",1,&[],"R",None,false,false,"(problem: view[Failure]) -> Option[Error]"),
    op!(TaskDiscard,Task,"discard",1,&[],"M",None,false,false,"(task: Task[T]) -> unit"),
    op!(TaskKind,Task,"kind",1,&[],"R",None,false,false,"(failure: view[TaskFailure]) -> TaskFailureKind"),
    op!(TaskMessage,Task,"message",1,&[],"R",Some(0),false,false,"(failure: view[TaskFailure]) -> view[str]"),
    op!(Status,HttpServer,"status",1,&[],"M",None,false,true,"(value: i64) -> Result[Status, Error]"),
    op!(Method,HttpServer,"method",1,&[],"R",None,false,true,"(name: view[str]) -> Result[Method, Error]"),
    op!(MethodName,HttpServer,"method_name",1,&[],"R",Some(0),false,true,"(method: view[Method]) -> view[str]"),
    op!(Empty,HttpServer,"empty",1,&[],"M",None,false,true,"(status: Status) -> Response"),
    op!(Text,HttpServer,"text",2,&[],"MR",None,false,true,"(status: Status, body: view[str]) -> Response"),
    op!(Html,HttpServer,"html",2,&[],"MR",None,false,true,"(status: Status, body: view[str]) -> Response"),
    op!(Bytes,HttpServer,"bytes",2,&[],"MR",None,false,true,"(status: Status, body: view[bytes]) -> Response"),
    op!(Json,HttpServer,"json",2,&["T"],"MB",None,false,true,"[T](status: Status, value: T) -> Result[Response, Error]"),
    op!(AppendHeader,HttpServer,"append_header",3,&[],"MRR",None,false,true,"(response: Response, name: view[str], value: view[bytes]) -> Result[Response, Error]"),
    op!(AppendHeaderText,HttpServer,"append_header_text",3,&[],"MRR",None,false,true,"(response: Response, name: view[str], value: view[str]) -> Result[Response, Error]"),
    op!(Header,HttpServer,"header",2,&[],"RR",Some(0),false,true,"(request: view[Request], name: view[str]) -> Result[Option[view[bytes]], Error]"),
    op!(HeaderText,HttpServer,"header_text",2,&[],"RR",Some(0),false,true,"(request: view[Request], name: view[str]) -> Result[Option[view[str]], Error]"),
    op!(Headers,HttpServer,"headers",2,&[],"RR",Some(0),false,true,"(request: view[Request], name: view[str]) -> Result[List[view[bytes]], Error]"),
    op!(IsJsonContentType,HttpServer,"is_json_content_type",1,&[],"R",None,false,true,"(request: view[Request]) -> Result[bool, Error]"),
    op!(DefaultOptions,HttpServer,"default_options",0,&[],"",None,false,true,"() -> Options"),
    op!(Options,HttpServer,"options",4,&[],"MMMM",None,false,true,"(body_bytes: i64, body_ms: i64, handler_ms: i64, shutdown_ms: i64) -> Result[Options, Error]"),
    op!(Capacity,HttpServer,"capacity",3,&[],"MMM",None,false,true,"(options: Options, connections: i64, requests: i64) -> Result[Options, Error]"),
    op!(HeaderTimeout,HttpServer,"header_timeout",2,&[],"MM",None,false,true,"(options: Options, milliseconds: i64) -> Result[Options, Error]"),
    op!(HeaderLimits,HttpServer,"header_limits",3,&[],"MMM",None,false,true,"(options: Options, bytes: i64, count: i64) -> Result[Options, Error]"),
    op!(SendTimeout,HttpServer,"send_timeout",2,&[],"MM",None,false,true,"(options: Options, milliseconds: i64) -> Result[Options, Error]"),
    op!(Authority,HttpServer,"authority",5,&[],"MRMMM",None,false,true,"(options: Options, external_origin: view[str], authorities: List[str], entry_limit: i64, byte_limit: i64) -> Result[Options, Error]"),
    op!(TrustedProxy,HttpServer,"trusted_proxy",2,&[],"MM",None,false,true,"(options: Options, peer_ips: List[str]) -> Result[Options, Error]"),
    op!(App,HttpServer,"app",2,&["S","E"],"MP",None,false,true,"[S, E](state: S, mapper: fn[E, Response]) -> App[S, E]"),
    op!(AppDefault,HttpServer,"app_default",1,&["S"],"M",None,false,true,"[S](state: S) -> App[S, Error]"),
    op!(Route,HttpServer,"route",5,&[],"MMRMH",None,false,false,"(app: App[S, E], method: Method, path: view[str], policy: Policy[S, A], handler: fn[Request, shared[S], A, Future[Result[Response, E]]]) -> Result[App[S, E], Error]"),
    op!(RouteMapped,HttpServer,"route_mapped",6,&[],"MMRMHP",None,false,false,"(app: App[S, E], method: Method, path: view[str], policy: Policy[S, A], handler: fn[Request, shared[S], A, Future[Result[Response, F]]], mapper: fn[F, Response]) -> Result[App[S, E], Error]"),
    op!(Serve,HttpServer,"serve",3,&[],"MMM",None,true,true,"(app: App[S, E], port: i64, options: Options) -> Future[Result[unit, Error]]"),
    op!(ActorDefaultOptions,Actor,"default_options",0,&[],"",None,false,true,"() -> Options"),
    op!(ActorOptions,Actor,"options",5,&[],"MMMMM",None,false,true,"(children: i64, events: i64, restarts: i64, window_ms: i64, shutdown_ms: i64) -> Result[Options, Error]"),
    op!(ActorDefaultActorOptions,Actor,"default_actor_options",0,&[],"",None,false,true,"() -> ActorOptions"),
    op!(ActorActorOptions,Actor,"actor_options",5,&[],"MMMMM",None,false,true,"(messages: i64, message_bytes: i64, reply_bytes: i64, startup_ms: i64, policy: RestartPolicy) -> Result[ActorOptions, Error]"),
    op!(ActorRestartDelay,Actor,"restart_delay",2,&[],"MM",None,false,true,"(options: Options, milliseconds: i64) -> Result[Options, Error]"),
    op!(ActorSupervisor,Actor,"supervisor",2,&["C"],"MM",None,false,true,"[C](context: C, options: Options) -> Supervisor[C]"),
    op!(ActorControl,Actor,"control",1,&[],"R",None,false,false,"(group: view[Supervisor[C]]) -> Control"),
    op!(ActorCloneControl,Actor,"clone_control",1,&[],"R",None,false,true,"(control: view[Control]) -> Control"),
    op!(ActorRegister,Actor,"register",5,&["S","M","R","E"],"RRHHM",None,false,false,"[S, M, R, E](group: view[Supervisor[C]], name: view[str], factory: fn[shared[C], Future[Result[S, Error]]], handler: fn[S, M, Future[Result[Turn[S, R, E], Error]]], options: ActorOptions) -> Result[Actor[M, R, E], Error]"),
    op!(ActorTask,Actor,"task",4,&[],"RRHM",None,false,false,"(group: view[Supervisor[C]], name: view[str], factory: fn[shared[C], Future[Result[unit, Error]]], policy: RestartPolicy) -> Result[unit, Error]"),
    op!(ActorTaskWithReady,Actor,"task_with_ready",4,&[],"RRHM",None,false,false,"(group: view[Supervisor[C]], name: view[str], factory: fn[shared[C], TaskReady, Future[Result[unit, Error]]], policy: RestartPolicy) -> Result[unit, Error]"),
    op!(ActorMarkReady,Actor,"mark_ready",1,&[],"R",None,false,false,"(signal: view[TaskReady]) -> Result[unit, Error]"),
    op!(ActorNextEventTimeout,Actor,"next_event_timeout",2,&[],"RM",None,true,false,"(control: view[Control], timeout_ms: i64) -> Future[Result[Option[Event], WaitError]]"),
    op!(ActorTurn,Actor,"turn",2,&["S","R","E"],"MM",None,false,true,"[S, R, E](state: S, reply: Result[R, E]) -> Turn[S, R, E]"),
    op!(ActorCloneActor,Actor,"clone_actor",1,&["M","R","E"],"R",None,false,true,"[M, R, E](actor: view[Actor[M, R, E]]) -> Actor[M, R, E]"),
    op!(ActorReady,Actor,"ready",2,&["M","R","E"],"RM",None,true,true,"[M, R, E](actor: view[Actor[M, R, E]], timeout_ms: i64) -> Future[Result[unit, CallError]]"),
    op!(ActorCall,Actor,"call",4,&["M","R","E"],"RMMM",None,true,true,"[M, R, E](actor: view[Actor[M, R, E]], message: M, mailbox_ms: i64, reply_ms: i64) -> Future[Result[Result[R, E], CallError]]"),
    op!(ActorRun,Actor,"run",1,&["C"],"M",None,true,true,"[C](group: Supervisor[C]) -> Future[Result[unit, Error]]"),
    op!(ActorShutdown,Actor,"shutdown",1,&[],"R",None,true,true,"(control: view[Control]) -> Future[Result[unit, Error]]"),
    op!(ActorNextEvent,Actor,"next_event",1,&[],"R",None,true,true,"(control: view[Control]) -> Future[Result[Option[Event], Error]]"),
    op!(ActorYieldNow,Actor,"yield_now",0,&[],"",None,true,true,"() -> Future[unit]"),
    op!(OwnershipMove,Ownership,"move",1,&[],"M",None,false,false,"(value: T) -> T"),
    op!(ResultMapError,Result,"map_error",2,&[],"MP",None,false,false,"(value: Result[T, E], mapper: fn[E, F]) -> Result[T, F]"),
];
#[test]
fn registered_operations_match_signatures_and_passing() {
    assert_eq!(OPERATIONS.len(), 83);
    assert_eq!(stdlib::OPERATIONS.len(), 83);
    assert_eq!(
        stdlib::OPERATIONS.iter().copied().collect::<HashSet<_>>(),
        OPERATIONS.iter().map(|r| r.operation).collect()
    );
    let mut names = HashSet::new();
    for expected in OPERATIONS {
        let actual = stdlib::operation_info(expected.operation);
        let (_, id, native) = module(expected.module);
        let passing: Vec<_> = expected
            .passing
            .chars()
            .map(|p| match p {
                'M' => Passing::Move,
                'B' => Passing::Borrow,
                'R' => Passing::Reference,
                'H' => Passing::Handler,
                'P' => Passing::Mapper,
                _ => panic!("invalid oracle Passing"),
            })
            .collect();
        assert_eq!(actual.module, expected.module);
        assert_eq!(actual.name, expected.name);
        // moveはchecked identity action。宣言metadataのnative pathだけidentityを指す。
        let rust_path = match expected.operation {
            O::OwnershipMove => "::std::convert::identity".to_owned(),
            O::TaskDiscard => "::nagi_runtime::TaskScope::discard".to_owned(),
            O::TaskKind => "::nagi_runtime::TaskFailure::kind".to_owned(),
            O::TaskMessage => "::nagi_runtime::TaskFailure::message".to_owned(),
            _ => format!("{native}::{}", expected.name),
        };
        assert_eq!(actual.rust_path, rust_path);
        assert_eq!(actual.arity, expected.arity, "{:?}", expected.operation);
        assert_eq!(actual.parameters, passing, "{:?}", expected.operation);
        assert_eq!(passing.len(), expected.arity);
        assert_eq!(actual.generic_arity, expected.generics.len());
        assert_eq!(actual.type_parameters, expected.generics);
        assert_eq!(actual.borrow_owner, expected.borrow);
        assert!(expected.borrow.is_none_or(|i| i < expected.arity));
        assert_eq!(actual.asynchronous, expected.asynchronous);
        assert_eq!(actual.emit_type_arguments, expected.emit_generics);
        assert_eq!(actual.signature, expected.signature);
        // Independently count top-level value parameters, including nested fn
        // and Result signatures without confusing their commas with arguments.
        let parameters = expected
            .signature
            .split_once('(')
            .unwrap()
            .1
            .split_once(") ->")
            .unwrap()
            .0;
        let mut depth = 0usize;
        let mut count = usize::from(!parameters.is_empty());
        for ch in parameters.chars() {
            match ch {
                '[' => depth += 1,
                ']' => depth = depth.checked_sub(1).unwrap(),
                ',' if depth == 0 => count += 1,
                _ => {}
            }
        }
        assert_eq!(depth, 0);
        assert_eq!(count, expected.arity);
        assert_eq!(
            stdlib::operation_module(expected.operation),
            ModuleId(id.into())
        );
        assert!(names.insert((id, expected.name)));
    }
}

// Resource-valued fields are written as nominal names and resolved only after
// their independent resource identity oracle above has been checked.
const FIELDS: &[(R, &str, &str, [bool; 3])] = &[
    (
        R::SqliteFailure,
        "kind",
        "SqliteFailureKind",
        [false, true, false],
    ),
    (
        R::SqliteFailure,
        "outcome",
        "SqliteOutcome",
        [false, true, false],
    ),
    (R::SqliteFailure, "retired", "bool", [false, true, false]),
    (
        R::SqliteFailure,
        "message",
        "view[str]",
        [false, true, false],
    ),
    (R::Request, "method", "Method", [true, false, false]),
    (R::Request, "path", "view[str]", [false, true, false]),
    (
        R::Request,
        "query",
        "Option[view[str]]",
        [false, true, false],
    ),
    (R::Request, "body", "view[bytes]", [false, true, false]),
    (R::Request, "is_get", "bool", [false, true, false]),
    (R::Request, "is_post", "bool", [false, true, false]),
    (R::Request, "is_put", "bool", [false, true, false]),
    (R::Request, "is_delete", "bool", [false, true, false]),
    (R::Request, "is_patch", "bool", [false, true, false]),
    (R::Request, "is_head", "bool", [false, true, false]),
    (R::Request, "is_options", "bool", [false, true, false]),
    (R::Request, "is_connect", "bool", [false, true, false]),
    (R::Request, "is_trace", "bool", [false, true, false]),
    (R::Response, "status", "Status", [false, true, false]),
    (R::Response, "body", "view[bytes]", [false, true, false]),
    (R::Status, "value", "i64", [false, true, false]),
    (R::Status, "phrase", "view[str]", [false, true, true]),
    (R::Status, "is_success", "bool", [false, true, false]),
    (R::Status, "is_redirection", "bool", [false, true, false]),
    (R::Status, "is_client_error", "bool", [false, true, false]),
    (R::Status, "is_server_error", "bool", [false, true, false]),
    (R::WaitError, "kind", "WaitKind", [false, true, false]),
    (R::WaitError, "message", "view[str]", [false, true, false]),
    (R::CallError, "kind", "CallKind", [false, true, false]),
    (R::CallError, "message", "view[str]", [false, true, false]),
    (R::Event, "child_id", "i64", [false, true, false]),
    (R::Event, "child_name", "view[str]", [false, true, false]),
    (R::Event, "generation", "i64", [false, true, false]),
    (R::Event, "kind", "EventKind", [false, true, false]),
    (R::Event, "message", "view[str]", [false, true, false]),
    (R::Event, "truncated", "bool", [false, true, false]),
    (R::Event, "lost_events", "i64", [false, true, false]),
];
fn field_type(ty: &str) -> Type {
    match ty {
        "SqliteFailureKind" => stdlib::resource_type(R::SqliteFailureKind, vec![]),
        "SqliteOutcome" => stdlib::resource_type(R::SqliteOutcome, vec![]),
        "Method" => stdlib::resource_type(R::Method, vec![]),
        "Status" => stdlib::resource_type(R::Status, vec![]),
        "WaitKind" => stdlib::resource_type(R::WaitKind, vec![]),
        "CallKind" => stdlib::resource_type(R::CallKind, vec![]),
        "EventKind" => stdlib::resource_type(R::EventKind, vec![]),
        "view[str]" => Type::generic("view", vec![Type::named("str")]),
        "view[bytes]" => Type::generic("view", vec![Type::named("bytes")]),
        "Option[view[str]]" => Type::generic(
            "Option",
            vec![Type::generic("view", vec![Type::named("str")])],
        ),
        "i64" | "bool" => Type::named(ty),
        _ => panic!("unknown oracle field type"),
    }
}
#[test]
fn registered_accessors_match_the_complete_inventory() {
    assert_eq!(FIELDS.len(), 36);
    assert_eq!(
        FIELDS
            .iter()
            .map(|r| (r.0, r.1))
            .collect::<HashSet<_>>()
            .len(),
        36
    );
    let mut total = 0;
    for &(resource, ..) in RESOURCES {
        let actual = stdlib::fields(resource);
        let expected: HashSet<_> = FIELDS
            .iter()
            .filter(|r| r.0 == resource)
            .map(|r| r.1)
            .collect();
        assert_eq!(
            actual.iter().map(|r| r.0).collect::<HashSet<_>>(),
            expected,
            "{resource:?}"
        );
        assert_eq!(actual.len(), expected.len());
        assert!(stdlib::field(resource, "not_an_accessor").is_none());
        total += actual.len();
    }
    assert_eq!(total, 36);
    for &(resource, name, ty, flags) in FIELDS {
        let actual = stdlib::field(resource, name).unwrap();
        assert_eq!(actual.ty, field_type(ty), "{resource:?}.{name}");
        assert_eq!(actual.accessor, name);
        assert_eq!(
            [actual.owned, actual.whole_owner, actual.static_borrow],
            flags
        );
    }
}

const CONSTANTS: &[(R, &str)] = &[
    (R::AuthFailureKind,"INVALID_CREDENTIAL DENIED EXPIRED INVALID_REQUEST UNAVAILABLE INTERNAL"),
    (R::SqliteBeginMode,"DEFERRED IMMEDIATE EXCLUSIVE"),
    (R::SqliteFailureKind,"INVALID CLOSED ACQUIRE_TIMEOUT BUSY SQL BIND DECODE ABORTED CLEANUP WORKER REPLY_LOST CLOSE_TIMEOUT ALLOCATION"),
    (R::SqliteOutcome,"NOT_APPLICABLE ACTIVE COMMITTED ROLLED_BACK UNKNOWN"),
    (R::TaskFailureKind,"Panicked Cancelled LegacyError Internal"),
    (R::Method,"GET POST PUT DELETE PATCH HEAD OPTIONS CONNECT TRACE"),
    (R::Status,"OK CREATED ACCEPTED NON_AUTHORITATIVE_INFORMATION NO_CONTENT RESET_CONTENT PARTIAL_CONTENT MULTI_STATUS ALREADY_REPORTED IM_USED MULTIPLE_CHOICES MOVED_PERMANENTLY FOUND SEE_OTHER NOT_MODIFIED USE_PROXY TEMPORARY_REDIRECT PERMANENT_REDIRECT BAD_REQUEST UNAUTHORIZED PAYMENT_REQUIRED FORBIDDEN NOT_FOUND METHOD_NOT_ALLOWED NOT_ACCEPTABLE PROXY_AUTHENTICATION_REQUIRED REQUEST_TIMEOUT CONFLICT GONE LENGTH_REQUIRED PRECONDITION_FAILED PAYLOAD_TOO_LARGE REQUEST_ENTITY_TOO_LARGE REQUEST_URI_TOO_LONG REQUESTED_RANGE_NOT_SATISFIABLE CONTENT_TOO_LARGE URI_TOO_LONG UNSUPPORTED_MEDIA_TYPE RANGE_NOT_SATISFIABLE EXPECTATION_FAILED IM_A_TEAPOT MISDIRECTED_REQUEST UNPROCESSABLE_ENTITY UNPROCESSABLE_CONTENT LOCKED FAILED_DEPENDENCY TOO_EARLY UPGRADE_REQUIRED PRECONDITION_REQUIRED TOO_MANY_REQUESTS REQUEST_HEADER_FIELDS_TOO_LARGE UNAVAILABLE_FOR_LEGAL_REASONS INTERNAL_SERVER_ERROR NOT_IMPLEMENTED BAD_GATEWAY SERVICE_UNAVAILABLE GATEWAY_TIMEOUT HTTP_VERSION_NOT_SUPPORTED VARIANT_ALSO_NEGOTIATES INSUFFICIENT_STORAGE LOOP_DETECTED NOT_EXTENDED NETWORK_AUTHENTICATION_REQUIRED"),
    (R::RestartPolicy,"TEMPORARY TRANSIENT PERMANENT"),
    (R::CallKind,"NOT_READY MAILBOX_FULL MAILBOX_TIMEOUT MESSAGE_TOO_LARGE REPLY_TOO_LARGE STOPPED RESTARTING REPLY_LOST REPLY_TIMEOUT"),
    (R::WaitKind,"TIMEOUT INVALID_TIMEOUT"),
    (R::EventKind,"READY STARTING STARTED FAILED PANICKED RESTART_SCHEDULED STOPPED INTENSITY_EXCEEDED SHUTDOWN LAGGED"),
];
#[test]
fn registered_constants_match_the_complete_inventory() {
    assert_eq!(CONSTANTS.len(), 11);
    for &(resource, ..) in RESOURCES {
        let names = CONSTANTS
            .iter()
            .find(|r| r.0 == resource)
            .map_or("", |r| r.1);
        let expected: HashSet<_> = names.split_whitespace().collect();
        assert_eq!(expected.len(), names.split_whitespace().count());
        let actual = stdlib::constants(resource);
        assert_eq!(
            actual.iter().map(|c| c.name).collect::<HashSet<_>>(),
            expected,
            "{resource:?}"
        );
        assert_eq!(actual.len(), expected.len());
        for name in expected {
            let native = match (resource, name) {
                (R::SqliteBeginMode, "DEFERRED") => "Deferred",
                (R::SqliteBeginMode, "IMMEDIATE") => "Immediate",
                (R::SqliteBeginMode, "EXCLUSIVE") => "Exclusive",
                (R::SqliteFailureKind, "INVALID") => "Invalid",
                (R::SqliteFailureKind, "CLOSED") => "Closed",
                (R::SqliteFailureKind, "ACQUIRE_TIMEOUT") => "AcquireTimeout",
                (R::SqliteFailureKind, "BUSY") => "Busy",
                (R::SqliteFailureKind, "SQL") => "Sql",
                (R::SqliteFailureKind, "BIND") => "Bind",
                (R::SqliteFailureKind, "DECODE") => "Decode",
                (R::SqliteFailureKind, "ABORTED") => "Aborted",
                (R::SqliteFailureKind, "CLEANUP") => "Cleanup",
                (R::SqliteFailureKind, "WORKER") => "Worker",
                (R::SqliteFailureKind, "REPLY_LOST") => "ReplyLost",
                (R::SqliteFailureKind, "CLOSE_TIMEOUT") => "CloseTimeout",
                (R::SqliteFailureKind, "ALLOCATION") => "Allocation",
                (R::SqliteOutcome, "NOT_APPLICABLE") => "NotApplicable",
                (R::SqliteOutcome, "ACTIVE") => "Active",
                (R::SqliteOutcome, "COMMITTED") => "Committed",
                (R::SqliteOutcome, "ROLLED_BACK") => "RolledBack",
                (R::SqliteOutcome, "UNKNOWN") => "Unknown",
                (R::Status, "REQUEST_ENTITY_TOO_LARGE") => "PAYLOAD_TOO_LARGE",
                (R::Status, "REQUEST_URI_TOO_LONG") => "URI_TOO_LONG",
                (R::Status, "REQUESTED_RANGE_NOT_SATISFIABLE") => "RANGE_NOT_SATISFIABLE",
                _ => name,
            };
            let actual = stdlib::constant(resource, name).unwrap();
            assert_eq!(actual.name, name);
            assert_eq!(actual.native_name, native);
        }
        assert!(stdlib::constant(resource, "NOT_A_REGISTERED_CONSTANT").is_none());
    }
}
