//! Compiler-owned standard definitions. Serialized Low names this registry;
//! it cannot choose native paths, capabilities, or callback contracts.
use crate::ast::*;

pub const MODULE_NAME: &str = "std.http.server";
pub const MODULE_ID: &str = "stdlib:std.http.server";
pub const ACTOR_MODULE_NAME: &str = "std.actor";
pub const ACTOR_MODULE_ID: &str = "stdlib:std.actor";
pub const RESULT_MODULE_NAME: &str = "std.result";
pub const RESULT_MODULE_ID: &str = "stdlib:std.result";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StandardModule {
    HttpServer,
    Actor,
    Result,
}
pub struct StandardModuleInfo {
    pub name: &'static str,
    pub id: &'static str,
    pub rust_namespace: &'static str,
}
pub const MODULES: &[StandardModule] = &[
    StandardModule::HttpServer,
    StandardModule::Actor,
    StandardModule::Result,
];
pub fn module_info(module: StandardModule) -> &'static StandardModuleInfo {
    match module {
        StandardModule::HttpServer => &StandardModuleInfo {
            name: MODULE_NAME,
            id: MODULE_ID,
            rust_namespace: "::nagi_runtime::http_server",
        },
        StandardModule::Actor => &StandardModuleInfo {
            name: ACTOR_MODULE_NAME,
            id: ACTOR_MODULE_ID,
            rust_namespace: "::nagi_runtime::actor",
        },
        StandardModule::Result => &StandardModuleInfo {
            name: RESULT_MODULE_NAME,
            id: RESULT_MODULE_ID,
            rust_namespace: "::nagi_runtime::result",
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Resource {
    Request,
    Response,
    Method,
    Status,
    Options,
    App,
    Supervisor,
    Control,
    Actor,
    Turn,
    SupervisorOptions,
    ActorOptions,
    RestartPolicy,
    CallKind,
    EventKind,
    CallError,
    Event,
    TaskReady,
    WaitKind,
    WaitError,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Operation {
    Status,
    Method,
    MethodName,
    Empty,
    Text,
    Html,
    Bytes,
    Json,
    AppendHeader,
    AppendHeaderText,
    Header,
    HeaderText,
    Headers,
    IsJsonContentType,
    DefaultOptions,
    Options,
    Capacity,
    HeaderTimeout,
    HeaderLimits,
    SendTimeout,
    App,
    AppDefault,
    Route,
    RouteMapped,
    Serve,
    ActorDefaultOptions,
    ActorOptions,
    ActorDefaultActorOptions,
    ActorActorOptions,
    ActorRestartDelay,
    ActorSupervisor,
    ActorControl,
    ActorCloneControl,
    ActorRegister,
    ActorTask,
    ActorTurn,
    ActorCloneActor,
    ActorReady,
    ActorCall,
    ActorRun,
    ActorShutdown,
    ActorNextEvent,
    ActorNextEventTimeout,
    ActorTaskWithReady,
    ActorMarkReady,
    ActorYieldNow,
    ResultMapError,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Passing {
    Move,
    Borrow,
    Reference,
    Handler,
    Mapper,
}
#[derive(Debug)]
pub struct ResourceInfo {
    pub module: StandardModule,
    pub name: &'static str,
    pub arity: usize,
    pub type_parameters: &'static [&'static str],
    pub inline_type_arguments: &'static [usize],
    pub rust_path: &'static str,
    pub copy: bool,
    pub equality: bool,
    pub storage: bool,
    pub shared: bool,
    pub debug: bool,
}
#[derive(Debug)]
pub struct OperationInfo {
    pub module: StandardModule,
    pub name: &'static str,
    pub rust_path: &'static str,
    pub arity: usize,
    pub generic_arity: usize,
    pub type_parameters: &'static [&'static str],
    pub asynchronous: bool,
    pub emit_type_arguments: bool,
    pub parameters: &'static [Passing],
    pub borrow_owner: Option<usize>,
    pub signature: &'static str,
}
#[derive(Clone, Debug)]
pub struct FieldInfo {
    pub ty: Type,
    pub accessor: &'static str,
    pub owned: bool,
    pub whole_owner: bool,
    pub static_borrow: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct ConstantInfo {
    pub name: &'static str,
    pub native_name: &'static str,
}

pub const RESOURCES: &[Resource] = &[
    Resource::Request,
    Resource::Response,
    Resource::Method,
    Resource::Status,
    Resource::Options,
    Resource::App,
    Resource::Supervisor,
    Resource::Control,
    Resource::Actor,
    Resource::Turn,
    Resource::SupervisorOptions,
    Resource::ActorOptions,
    Resource::RestartPolicy,
    Resource::CallKind,
    Resource::EventKind,
    Resource::CallError,
    Resource::Event,
    Resource::TaskReady,
    Resource::WaitKind,
    Resource::WaitError,
];
pub const OPERATIONS: &[Operation] = &[
    Operation::Status,
    Operation::Method,
    Operation::MethodName,
    Operation::Empty,
    Operation::Text,
    Operation::Html,
    Operation::Bytes,
    Operation::Json,
    Operation::AppendHeader,
    Operation::AppendHeaderText,
    Operation::Header,
    Operation::HeaderText,
    Operation::Headers,
    Operation::IsJsonContentType,
    Operation::DefaultOptions,
    Operation::Options,
    Operation::Capacity,
    Operation::HeaderTimeout,
    Operation::HeaderLimits,
    Operation::SendTimeout,
    Operation::App,
    Operation::AppDefault,
    Operation::Route,
    Operation::RouteMapped,
    Operation::Serve,
    Operation::ActorDefaultOptions,
    Operation::ActorOptions,
    Operation::ActorDefaultActorOptions,
    Operation::ActorActorOptions,
    Operation::ActorRestartDelay,
    Operation::ActorSupervisor,
    Operation::ActorControl,
    Operation::ActorCloneControl,
    Operation::ActorRegister,
    Operation::ActorTask,
    Operation::ActorTurn,
    Operation::ActorCloneActor,
    Operation::ActorReady,
    Operation::ActorCall,
    Operation::ActorRun,
    Operation::ActorShutdown,
    Operation::ActorNextEvent,
    Operation::ActorNextEventTimeout,
    Operation::ActorTaskWithReady,
    Operation::ActorMarkReady,
    Operation::ActorYieldNow,
    Operation::ResultMapError,
];

pub fn module(name: &str) -> Option<ModuleId> {
    MODULES.iter().find_map(|module| {
        let info = module_info(*module);
        (name == info.name).then(|| ModuleId(info.id.into()))
    })
}
pub fn is_registered_module(id: &ModuleId) -> bool {
    MODULES.iter().any(|module| module_info(*module).id == id.0)
}
pub fn resource_info(resource: Resource) -> &'static ResourceInfo {
    match resource {
        Resource::Request => &ResourceInfo {
            module: StandardModule::HttpServer,
            type_parameters: &[],
            inline_type_arguments: &[],
            name: "Request",
            arity: 0,
            rust_path: "::nagi_runtime::http_server::Request",
            copy: false,
            equality: false,
            storage: true,
            shared: true,
            debug: true,
        },
        Resource::Response => &ResourceInfo {
            module: StandardModule::HttpServer,
            type_parameters: &[],
            inline_type_arguments: &[],
            name: "Response",
            arity: 0,
            rust_path: "::nagi_runtime::http_server::Response",
            copy: false,
            equality: false,
            storage: true,
            shared: false,
            debug: true,
        },
        Resource::Method => &ResourceInfo {
            module: StandardModule::HttpServer,
            type_parameters: &[],
            inline_type_arguments: &[],
            name: "Method",
            arity: 0,
            rust_path: "::nagi_runtime::http_server::Method",
            copy: false,
            equality: true,
            storage: true,
            shared: true,
            debug: true,
        },
        Resource::Status => &ResourceInfo {
            module: StandardModule::HttpServer,
            type_parameters: &[],
            inline_type_arguments: &[],
            name: "Status",
            arity: 0,
            rust_path: "::nagi_runtime::http_server::Status",
            copy: true,
            equality: true,
            storage: true,
            shared: true,
            debug: true,
        },
        Resource::Options => &ResourceInfo {
            module: StandardModule::HttpServer,
            type_parameters: &[],
            inline_type_arguments: &[],
            name: "Options",
            arity: 0,
            rust_path: "::nagi_runtime::http_server::Options",
            copy: false,
            equality: false,
            storage: true,
            shared: true,
            debug: true,
        },
        Resource::App => &ResourceInfo {
            module: StandardModule::HttpServer,
            type_parameters: &["S", "E"],
            inline_type_arguments: &[],
            name: "App",
            arity: 2,
            rust_path: "::nagi_runtime::http_server::App",
            copy: false,
            equality: false,
            storage: true,
            shared: false,
            debug: true,
        },
        Resource::Supervisor => &ResourceInfo {
            module: StandardModule::Actor,
            name: "Supervisor",
            arity: 1,
            type_parameters: &["C"],
            inline_type_arguments: &[],
            rust_path: "::nagi_runtime::actor::Supervisor",
            copy: false,
            equality: false,
            storage: true,
            shared: false,
            debug: true,
        },
        Resource::WaitKind => &ResourceInfo {
            module: StandardModule::Actor,
            name: "WaitKind",
            arity: 0,
            type_parameters: &[],
            inline_type_arguments: &[],
            rust_path: "::nagi_runtime::actor::WaitKind",
            copy: true,
            equality: true,
            storage: true,
            shared: true,
            debug: true,
        },
        Resource::WaitError => &ResourceInfo {
            module: StandardModule::Actor,
            name: "WaitError",
            arity: 0,
            type_parameters: &[],
            inline_type_arguments: &[],
            rust_path: "::nagi_runtime::actor::WaitError",
            copy: false,
            equality: false,
            storage: true,
            shared: true,
            debug: true,
        },
        Resource::TaskReady => &ResourceInfo {
            module: StandardModule::Actor,
            name: "TaskReady",
            arity: 0,
            type_parameters: &[],
            inline_type_arguments: &[],
            rust_path: "::nagi_runtime::actor::TaskReady",
            copy: false,
            equality: false,
            storage: true,
            shared: false,
            debug: true,
        },
        Resource::Control => &ResourceInfo {
            module: StandardModule::Actor,
            name: "Control",
            arity: 0,
            type_parameters: &[],
            inline_type_arguments: &[],
            rust_path: "::nagi_runtime::actor::Control",
            copy: false,
            equality: false,
            storage: true,
            shared: true,
            debug: true,
        },
        Resource::Actor => &ResourceInfo {
            module: StandardModule::Actor,
            name: "Actor",
            arity: 3,
            type_parameters: &["M", "R", "E"],
            inline_type_arguments: &[],
            rust_path: "::nagi_runtime::actor::Actor",
            copy: false,
            equality: false,
            storage: true,
            shared: true,
            debug: true,
        },
        Resource::Turn => &ResourceInfo {
            module: StandardModule::Actor,
            name: "Turn",
            arity: 3,
            type_parameters: &["S", "R", "E"],
            inline_type_arguments: &[0, 1, 2],
            rust_path: "::nagi_runtime::actor::Turn",
            copy: false,
            equality: false,
            storage: true,
            shared: false,
            debug: true,
        },
        Resource::SupervisorOptions => &ResourceInfo {
            module: StandardModule::Actor,
            name: "Options",
            arity: 0,
            type_parameters: &[],
            inline_type_arguments: &[],
            rust_path: "::nagi_runtime::actor::Options",
            copy: false,
            equality: false,
            storage: true,
            shared: true,
            debug: true,
        },
        Resource::ActorOptions => &ResourceInfo {
            module: StandardModule::Actor,
            name: "ActorOptions",
            arity: 0,
            type_parameters: &[],
            inline_type_arguments: &[],
            rust_path: "::nagi_runtime::actor::ActorOptions",
            copy: false,
            equality: false,
            storage: true,
            shared: true,
            debug: true,
        },
        Resource::RestartPolicy => &ResourceInfo {
            module: StandardModule::Actor,
            name: "RestartPolicy",
            arity: 0,
            type_parameters: &[],
            inline_type_arguments: &[],
            rust_path: "::nagi_runtime::actor::RestartPolicy",
            copy: true,
            equality: true,
            storage: true,
            shared: true,
            debug: true,
        },
        Resource::CallKind => &ResourceInfo {
            module: StandardModule::Actor,
            name: "CallKind",
            arity: 0,
            type_parameters: &[],
            inline_type_arguments: &[],
            rust_path: "::nagi_runtime::actor::CallKind",
            copy: true,
            equality: true,
            storage: true,
            shared: true,
            debug: true,
        },
        Resource::EventKind => &ResourceInfo {
            module: StandardModule::Actor,
            name: "EventKind",
            arity: 0,
            type_parameters: &[],
            inline_type_arguments: &[],
            rust_path: "::nagi_runtime::actor::EventKind",
            copy: true,
            equality: true,
            storage: true,
            shared: true,
            debug: true,
        },
        Resource::CallError => &ResourceInfo {
            module: StandardModule::Actor,
            name: "CallError",
            arity: 0,
            type_parameters: &[],
            inline_type_arguments: &[],
            rust_path: "::nagi_runtime::actor::CallError",
            copy: false,
            equality: false,
            storage: true,
            shared: true,
            debug: true,
        },
        Resource::Event => &ResourceInfo {
            module: StandardModule::Actor,
            name: "Event",
            arity: 0,
            type_parameters: &[],
            inline_type_arguments: &[],
            rust_path: "::nagi_runtime::actor::Event",
            copy: false,
            equality: false,
            storage: true,
            shared: true,
            debug: true,
        },
    }
}
pub fn operation_info(operation: Operation) -> &'static OperationInfo {
    match operation {
 Operation::Status => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "status", rust_path: "::nagi_runtime::http_server::status", arity: 1, generic_arity: 0, parameters: &[Passing::Move], borrow_owner: None, signature: "(value: i64) -> Result[Status, Error]" },
 Operation::Method => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "method", rust_path: "::nagi_runtime::http_server::method", arity: 1, generic_arity: 0, parameters: &[Passing::Reference], borrow_owner: None, signature: "(name: view[str]) -> Result[Method, Error]" },
 Operation::MethodName => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "method_name", rust_path: "::nagi_runtime::http_server::method_name", arity: 1, generic_arity: 0, parameters: &[Passing::Reference], borrow_owner: Some(0), signature: "(method: view[Method]) -> view[str]" },
 Operation::Empty => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "empty", rust_path: "::nagi_runtime::http_server::empty", arity: 1, generic_arity: 0, parameters: &[Passing::Move], borrow_owner: None, signature: "(status: Status) -> Response" },
 Operation::Text => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "text", rust_path: "::nagi_runtime::http_server::text", arity: 2, generic_arity: 0, parameters: &[Passing::Move, Passing::Reference], borrow_owner: None, signature: "(status: Status, body: view[str]) -> Response" },
 Operation::Html => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "html", rust_path: "::nagi_runtime::http_server::html", arity: 2, generic_arity: 0, parameters: &[Passing::Move, Passing::Reference], borrow_owner: None, signature: "(status: Status, body: view[str]) -> Response" },
 Operation::Bytes => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "bytes", rust_path: "::nagi_runtime::http_server::bytes", arity: 2, generic_arity: 0, parameters: &[Passing::Move, Passing::Reference], borrow_owner: None, signature: "(status: Status, body: view[bytes]) -> Response" },
 Operation::Json => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &["T"], asynchronous: false, emit_type_arguments: true, name: "json", rust_path: "::nagi_runtime::http_server::json", arity: 2, generic_arity: 1, parameters: &[Passing::Move, Passing::Borrow], borrow_owner: None, signature: "[T](status: Status, value: T) -> Result[Response, Error]" },
 Operation::AppendHeader => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "append_header", rust_path: "::nagi_runtime::http_server::append_header", arity: 3, generic_arity: 0, parameters: &[Passing::Move, Passing::Reference, Passing::Reference], borrow_owner: None, signature: "(response: Response, name: view[str], value: view[bytes]) -> Result[Response, Error]" },
 Operation::AppendHeaderText => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "append_header_text", rust_path: "::nagi_runtime::http_server::append_header_text", arity: 3, generic_arity: 0, parameters: &[Passing::Move, Passing::Reference, Passing::Reference], borrow_owner: None, signature: "(response: Response, name: view[str], value: view[str]) -> Result[Response, Error]" },
 Operation::Header => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "header", rust_path: "::nagi_runtime::http_server::header", arity: 2, generic_arity: 0, parameters: &[Passing::Reference, Passing::Reference], borrow_owner: Some(0), signature: "(request: view[Request], name: view[str]) -> Result[Option[view[bytes]], Error]" },
 Operation::HeaderText => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "header_text", rust_path: "::nagi_runtime::http_server::header_text", arity: 2, generic_arity: 0, parameters: &[Passing::Reference, Passing::Reference], borrow_owner: Some(0), signature: "(request: view[Request], name: view[str]) -> Result[Option[view[str]], Error]" },
 Operation::Headers => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "headers", rust_path: "::nagi_runtime::http_server::headers", arity: 2, generic_arity: 0, parameters: &[Passing::Reference, Passing::Reference], borrow_owner: Some(0), signature: "(request: view[Request], name: view[str]) -> Result[List[view[bytes]], Error]" },
 Operation::IsJsonContentType => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "is_json_content_type", rust_path: "::nagi_runtime::http_server::is_json_content_type", arity: 1, generic_arity: 0, parameters: &[Passing::Reference], borrow_owner: None, signature: "(request: view[Request]) -> Result[bool, Error]" },
 Operation::DefaultOptions => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "default_options", rust_path: "::nagi_runtime::http_server::default_options", arity: 0, generic_arity: 0, parameters: &[], borrow_owner: None, signature: "() -> Options" },
 Operation::Options => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "options", rust_path: "::nagi_runtime::http_server::options", arity: 4, generic_arity: 0, parameters: &[Passing::Move, Passing::Move, Passing::Move, Passing::Move], borrow_owner: None, signature: "(body_bytes: i64, body_ms: i64, handler_ms: i64, shutdown_ms: i64) -> Result[Options, Error]" },
 Operation::Capacity => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "capacity", rust_path: "::nagi_runtime::http_server::capacity", arity: 3, generic_arity: 0, parameters: &[Passing::Move, Passing::Move, Passing::Move], borrow_owner: None, signature: "(options: Options, connections: i64, requests: i64) -> Result[Options, Error]" },
 Operation::HeaderTimeout => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "header_timeout", rust_path: "::nagi_runtime::http_server::header_timeout", arity: 2, generic_arity: 0, parameters: &[Passing::Move, Passing::Move], borrow_owner: None, signature: "(options: Options, milliseconds: i64) -> Result[Options, Error]" },
 Operation::HeaderLimits => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "header_limits", rust_path: "::nagi_runtime::http_server::header_limits", arity: 3, generic_arity: 0, parameters: &[Passing::Move, Passing::Move, Passing::Move], borrow_owner: None, signature: "(options: Options, bytes: i64, count: i64) -> Result[Options, Error]" },
 Operation::SendTimeout => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: true, name: "send_timeout", rust_path: "::nagi_runtime::http_server::send_timeout", arity: 2, generic_arity: 0, parameters: &[Passing::Move, Passing::Move], borrow_owner: None, signature: "(options: Options, milliseconds: i64) -> Result[Options, Error]" },
 Operation::App => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &["S", "E"], asynchronous: false, emit_type_arguments: true, name: "app", rust_path: "::nagi_runtime::http_server::app", arity: 2, generic_arity: 2, parameters: &[Passing::Move, Passing::Mapper], borrow_owner: None, signature: "[S, E](state: S, mapper: fn[E, Response]) -> App[S, E]" },
 Operation::AppDefault => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &["S"], asynchronous: false, emit_type_arguments: true, name: "app_default", rust_path: "::nagi_runtime::http_server::app_default", arity: 1, generic_arity: 1, parameters: &[Passing::Move], borrow_owner: None, signature: "[S](state: S) -> App[S, Error]" },
 Operation::Route => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: false, name: "route", rust_path: "::nagi_runtime::http_server::route", arity: 4, generic_arity: 0, parameters: &[Passing::Move, Passing::Move, Passing::Reference, Passing::Handler], borrow_owner: None, signature: "(app: App[S, E], method: Method, path: view[str], handler: fn[Request, shared[S], Future[Result[Response, E]]]) -> Result[App[S, E], Error]" },
 Operation::RouteMapped => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: false, emit_type_arguments: false, name: "route_mapped", rust_path: "::nagi_runtime::http_server::route_mapped", arity: 5, generic_arity: 0, parameters: &[Passing::Move, Passing::Move, Passing::Reference, Passing::Handler, Passing::Mapper], borrow_owner: None, signature: "(app: App[S, E], method: Method, path: view[str], handler: fn[Request, shared[S], Future[Result[Response, F]]], mapper: fn[F, Response]) -> Result[App[S, E], Error]" },
 Operation::Serve => &OperationInfo { module: StandardModule::HttpServer, type_parameters: &[], asynchronous: true, emit_type_arguments: true, name: "serve", rust_path: "::nagi_runtime::http_server::serve", arity: 3, generic_arity: 0, parameters: &[Passing::Move, Passing::Move, Passing::Move], borrow_owner: None, signature: "(app: App[S, E], port: i64, options: Options) -> Future[Result[unit, Error]]" },
        Operation::ActorDefaultOptions => &OperationInfo { module: StandardModule::Actor, name: "default_options", rust_path: "::nagi_runtime::actor::default_options", arity: 0, generic_arity: 0, type_parameters: &[], asynchronous: false, emit_type_arguments: true, parameters: &[], borrow_owner: None, signature: "() -> Options" },
        Operation::ActorOptions => &OperationInfo { module: StandardModule::Actor, name: "options", rust_path: "::nagi_runtime::actor::options", arity: 5, generic_arity: 0, type_parameters: &[], asynchronous: false, emit_type_arguments: true, parameters: &[Passing::Move, Passing::Move, Passing::Move, Passing::Move, Passing::Move], borrow_owner: None, signature: "(children: i64, events: i64, restarts: i64, window_ms: i64, shutdown_ms: i64) -> Result[Options, Error]" },
        Operation::ActorDefaultActorOptions => &OperationInfo { module: StandardModule::Actor, name: "default_actor_options", rust_path: "::nagi_runtime::actor::default_actor_options", arity: 0, generic_arity: 0, type_parameters: &[], asynchronous: false, emit_type_arguments: true, parameters: &[], borrow_owner: None, signature: "() -> ActorOptions" },
        Operation::ActorActorOptions => &OperationInfo { module: StandardModule::Actor, name: "actor_options", rust_path: "::nagi_runtime::actor::actor_options", arity: 5, generic_arity: 0, type_parameters: &[], asynchronous: false, emit_type_arguments: true, parameters: &[Passing::Move, Passing::Move, Passing::Move, Passing::Move, Passing::Move], borrow_owner: None, signature: "(messages: i64, message_bytes: i64, reply_bytes: i64, startup_ms: i64, policy: RestartPolicy) -> Result[ActorOptions, Error]" },
        Operation::ActorRestartDelay => &OperationInfo { module: StandardModule::Actor, name: "restart_delay", rust_path: "::nagi_runtime::actor::restart_delay", arity: 2, generic_arity: 0, type_parameters: &[], asynchronous: false, emit_type_arguments: true, parameters: &[Passing::Move, Passing::Move], borrow_owner: None, signature: "(options: Options, milliseconds: i64) -> Result[Options, Error]" },
        Operation::ActorSupervisor => &OperationInfo { module: StandardModule::Actor, name: "supervisor", rust_path: "::nagi_runtime::actor::supervisor", arity: 2, generic_arity: 1, type_parameters: &["C"], asynchronous: false, emit_type_arguments: true, parameters: &[Passing::Move, Passing::Move], borrow_owner: None, signature: "[C](context: C, options: Options) -> Supervisor[C]" },
        Operation::ActorControl => &OperationInfo { module: StandardModule::Actor, name: "control", rust_path: "::nagi_runtime::actor::control", arity: 1, generic_arity: 0, type_parameters: &[], asynchronous: false, emit_type_arguments: false, parameters: &[Passing::Reference], borrow_owner: None, signature: "(group: view[Supervisor[C]]) -> Control" },
        Operation::ActorCloneControl => &OperationInfo { module: StandardModule::Actor, name: "clone_control", rust_path: "::nagi_runtime::actor::clone_control", arity: 1, generic_arity: 0, type_parameters: &[], asynchronous: false, emit_type_arguments: true, parameters: &[Passing::Reference], borrow_owner: None, signature: "(control: view[Control]) -> Control" },
        Operation::ActorRegister => &OperationInfo { module: StandardModule::Actor, name: "register", rust_path: "::nagi_runtime::actor::register", arity: 5, generic_arity: 4, type_parameters: &["S", "M", "R", "E"], asynchronous: false, emit_type_arguments: false, parameters: &[Passing::Reference, Passing::Reference, Passing::Handler, Passing::Handler, Passing::Move], borrow_owner: None, signature: "[S, M, R, E](group: view[Supervisor[C]], name: view[str], factory: fn[shared[C], Future[Result[S, Error]]], handler: fn[S, M, Future[Result[Turn[S, R, E], Error]]], options: ActorOptions) -> Result[Actor[M, R, E], Error]" },
        Operation::ActorTask => &OperationInfo { module: StandardModule::Actor, name: "task", rust_path: "::nagi_runtime::actor::task", arity: 4, generic_arity: 0, type_parameters: &[], asynchronous: false, emit_type_arguments: false, parameters: &[Passing::Reference, Passing::Reference, Passing::Handler, Passing::Move], borrow_owner: None, signature: "(group: view[Supervisor[C]], name: view[str], factory: fn[shared[C], Future[Result[unit, Error]]], policy: RestartPolicy) -> Result[unit, Error]" },
        Operation::ActorTaskWithReady => &OperationInfo { module: StandardModule::Actor, name: "task_with_ready", rust_path: "::nagi_runtime::actor::task_with_ready", arity: 4, generic_arity: 0, type_parameters: &[], asynchronous: false, emit_type_arguments: false, parameters: &[Passing::Reference, Passing::Reference, Passing::Handler, Passing::Move], borrow_owner: None, signature: "(group: view[Supervisor[C]], name: view[str], factory: fn[shared[C], TaskReady, Future[Result[unit, Error]]], policy: RestartPolicy) -> Result[unit, Error]" },
        Operation::ActorMarkReady => &OperationInfo { module: StandardModule::Actor, name: "mark_ready", rust_path: "::nagi_runtime::actor::mark_ready", arity: 1, generic_arity: 0, type_parameters: &[], asynchronous: false, emit_type_arguments: false, parameters: &[Passing::Reference], borrow_owner: None, signature: "(signal: view[TaskReady]) -> Result[unit, Error]" },
        Operation::ActorNextEventTimeout => &OperationInfo { module: StandardModule::Actor, name: "next_event_timeout", rust_path: "::nagi_runtime::actor::next_event_timeout", arity: 2, generic_arity: 0, type_parameters: &[], asynchronous: true, emit_type_arguments: false, parameters: &[Passing::Reference, Passing::Move], borrow_owner: None, signature: "(control: view[Control], timeout_ms: i64) -> Future[Result[Option[Event], WaitError]]" },
        Operation::ActorTurn => &OperationInfo { module: StandardModule::Actor, name: "turn", rust_path: "::nagi_runtime::actor::turn", arity: 2, generic_arity: 3, type_parameters: &["S", "R", "E"], asynchronous: false, emit_type_arguments: true, parameters: &[Passing::Move, Passing::Move], borrow_owner: None, signature: "[S, R, E](state: S, reply: Result[R, E]) -> Turn[S, R, E]" },
        Operation::ActorCloneActor => &OperationInfo { module: StandardModule::Actor, name: "clone_actor", rust_path: "::nagi_runtime::actor::clone_actor", arity: 1, generic_arity: 3, type_parameters: &["M", "R", "E"], asynchronous: false, emit_type_arguments: true, parameters: &[Passing::Reference], borrow_owner: None, signature: "[M, R, E](actor: view[Actor[M, R, E]]) -> Actor[M, R, E]" },
        Operation::ActorReady => &OperationInfo { module: StandardModule::Actor, name: "ready", rust_path: "::nagi_runtime::actor::ready", arity: 2, generic_arity: 3, type_parameters: &["M", "R", "E"], asynchronous: true, emit_type_arguments: true, parameters: &[Passing::Reference, Passing::Move], borrow_owner: None, signature: "[M, R, E](actor: view[Actor[M, R, E]], timeout_ms: i64) -> Future[Result[unit, CallError]]" },
        Operation::ActorCall => &OperationInfo { module: StandardModule::Actor, name: "call", rust_path: "::nagi_runtime::actor::call", arity: 4, generic_arity: 3, type_parameters: &["M", "R", "E"], asynchronous: true, emit_type_arguments: true, parameters: &[Passing::Reference, Passing::Move, Passing::Move, Passing::Move], borrow_owner: None, signature: "[M, R, E](actor: view[Actor[M, R, E]], message: M, mailbox_ms: i64, reply_ms: i64) -> Future[Result[Result[R, E], CallError]]" },
        Operation::ActorRun => &OperationInfo { module: StandardModule::Actor, name: "run", rust_path: "::nagi_runtime::actor::run", arity: 1, generic_arity: 1, type_parameters: &["C"], asynchronous: true, emit_type_arguments: true, parameters: &[Passing::Move], borrow_owner: None, signature: "[C](group: Supervisor[C]) -> Future[Result[unit, Error]]" },
        Operation::ActorShutdown => &OperationInfo { module: StandardModule::Actor, name: "shutdown", rust_path: "::nagi_runtime::actor::shutdown", arity: 1, generic_arity: 0, type_parameters: &[], asynchronous: true, emit_type_arguments: true, parameters: &[Passing::Reference], borrow_owner: None, signature: "(control: view[Control]) -> Future[Result[unit, Error]]" },
        Operation::ActorNextEvent => &OperationInfo { module: StandardModule::Actor, name: "next_event", rust_path: "::nagi_runtime::actor::next_event", arity: 1, generic_arity: 0, type_parameters: &[], asynchronous: true, emit_type_arguments: true, parameters: &[Passing::Reference], borrow_owner: None, signature: "(control: view[Control]) -> Future[Result[Option[Event], Error]]" },
        Operation::ActorYieldNow => &OperationInfo { module: StandardModule::Actor, name: "yield_now", rust_path: "::nagi_runtime::actor::yield_now", arity: 0, generic_arity: 0, type_parameters: &[], asynchronous: true, emit_type_arguments: true, parameters: &[], borrow_owner: None, signature: "() -> Future[unit]" },
        Operation::ResultMapError => &OperationInfo { module: StandardModule::Result, name: "map_error", rust_path: "::nagi_runtime::result::map_error", arity: 2, generic_arity: 0, type_parameters: &[], asynchronous: false, emit_type_arguments: false, parameters: &[Passing::Move, Passing::Mapper], borrow_owner: None, signature: "(value: Result[T, E], mapper: fn[E, F]) -> Result[T, F]" },
}
}
pub fn resource_id(resource: Resource) -> DefId {
    DefId {
        module: resource_module(resource),
        kind: DefKind::Resource,
        name: resource_info(resource).name.into(),
    }
}
pub fn function_id(operation: Operation) -> DefId {
    DefId {
        module: operation_module(operation),
        kind: DefKind::Function,
        name: operation_info(operation).name.into(),
    }
}
pub fn resource_module(resource: Resource) -> ModuleId {
    ModuleId(module_info(resource_info(resource).module).id.into())
}
pub fn operation_module(operation: Operation) -> ModuleId {
    ModuleId(module_info(operation_info(operation).module).id.into())
}
pub fn resource_named(module: &ModuleId, name: &str) -> Option<Resource> {
    RESOURCES.iter().copied().find(|resource| {
        resource_module(*resource) == *module && resource_info(*resource).name == name
    })
}
pub fn resource_type(resource: Resource, arguments: Vec<Type>) -> Type {
    Type(crate::modules::symbol(&resource_id(resource)), arguments)
}
/// Classifies native resource references without treating shared resources as
/// references. Callers must also verify the resource's metadata identity.
pub fn native_view_element(ty: &Type) -> Option<&Type> {
    if ty.0 != "view" || ty.1.len() != 1 {
        return None;
    }
    let mut element = &ty.1[0];
    while element.0 == "owned" && element.1.len() == 1 {
        element = &element.1[0];
    }
    resource(&element.0).map(|_| element)
}
pub fn resource(symbol: &str) -> Option<Resource> {
    RESOURCES
        .iter()
        .copied()
        .find(|r| crate::modules::symbol(&resource_id(*r)) == symbol)
}
pub fn operation(symbol: &str) -> Option<Operation> {
    OPERATIONS
        .iter()
        .copied()
        .find(|op| crate::modules::symbol(&function_id(*op)) == symbol)
}
pub fn definition(id: &DefId) -> bool {
    is_registered_module(&id.module)
        && (RESOURCES.iter().any(|r| resource_id(*r) == *id)
            || OPERATIONS.iter().any(|op| function_id(*op) == *id))
}
pub fn contains_symbol(symbol: &str) -> bool {
    resource(symbol).is_some() || operation(symbol).is_some()
}

const METHOD_CONSTANTS: &[ConstantInfo] = &[
    ConstantInfo {
        name: "GET",
        native_name: "GET",
    },
    ConstantInfo {
        name: "POST",
        native_name: "POST",
    },
    ConstantInfo {
        name: "PUT",
        native_name: "PUT",
    },
    ConstantInfo {
        name: "DELETE",
        native_name: "DELETE",
    },
    ConstantInfo {
        name: "PATCH",
        native_name: "PATCH",
    },
    ConstantInfo {
        name: "HEAD",
        native_name: "HEAD",
    },
    ConstantInfo {
        name: "OPTIONS",
        native_name: "OPTIONS",
    },
    ConstantInfo {
        name: "CONNECT",
        native_name: "CONNECT",
    },
    ConstantInfo {
        name: "TRACE",
        native_name: "TRACE",
    },
];
const STATUS_CONSTANTS: &[ConstantInfo] = &[
    ConstantInfo {
        name: "OK",
        native_name: "OK",
    },
    ConstantInfo {
        name: "CREATED",
        native_name: "CREATED",
    },
    ConstantInfo {
        name: "ACCEPTED",
        native_name: "ACCEPTED",
    },
    ConstantInfo {
        name: "NON_AUTHORITATIVE_INFORMATION",
        native_name: "NON_AUTHORITATIVE_INFORMATION",
    },
    ConstantInfo {
        name: "NO_CONTENT",
        native_name: "NO_CONTENT",
    },
    ConstantInfo {
        name: "RESET_CONTENT",
        native_name: "RESET_CONTENT",
    },
    ConstantInfo {
        name: "PARTIAL_CONTENT",
        native_name: "PARTIAL_CONTENT",
    },
    ConstantInfo {
        name: "MULTI_STATUS",
        native_name: "MULTI_STATUS",
    },
    ConstantInfo {
        name: "ALREADY_REPORTED",
        native_name: "ALREADY_REPORTED",
    },
    ConstantInfo {
        name: "IM_USED",
        native_name: "IM_USED",
    },
    ConstantInfo {
        name: "MULTIPLE_CHOICES",
        native_name: "MULTIPLE_CHOICES",
    },
    ConstantInfo {
        name: "MOVED_PERMANENTLY",
        native_name: "MOVED_PERMANENTLY",
    },
    ConstantInfo {
        name: "FOUND",
        native_name: "FOUND",
    },
    ConstantInfo {
        name: "SEE_OTHER",
        native_name: "SEE_OTHER",
    },
    ConstantInfo {
        name: "NOT_MODIFIED",
        native_name: "NOT_MODIFIED",
    },
    ConstantInfo {
        name: "USE_PROXY",
        native_name: "USE_PROXY",
    },
    ConstantInfo {
        name: "TEMPORARY_REDIRECT",
        native_name: "TEMPORARY_REDIRECT",
    },
    ConstantInfo {
        name: "PERMANENT_REDIRECT",
        native_name: "PERMANENT_REDIRECT",
    },
    ConstantInfo {
        name: "BAD_REQUEST",
        native_name: "BAD_REQUEST",
    },
    ConstantInfo {
        name: "UNAUTHORIZED",
        native_name: "UNAUTHORIZED",
    },
    ConstantInfo {
        name: "PAYMENT_REQUIRED",
        native_name: "PAYMENT_REQUIRED",
    },
    ConstantInfo {
        name: "FORBIDDEN",
        native_name: "FORBIDDEN",
    },
    ConstantInfo {
        name: "NOT_FOUND",
        native_name: "NOT_FOUND",
    },
    ConstantInfo {
        name: "METHOD_NOT_ALLOWED",
        native_name: "METHOD_NOT_ALLOWED",
    },
    ConstantInfo {
        name: "NOT_ACCEPTABLE",
        native_name: "NOT_ACCEPTABLE",
    },
    ConstantInfo {
        name: "PROXY_AUTHENTICATION_REQUIRED",
        native_name: "PROXY_AUTHENTICATION_REQUIRED",
    },
    ConstantInfo {
        name: "REQUEST_TIMEOUT",
        native_name: "REQUEST_TIMEOUT",
    },
    ConstantInfo {
        name: "CONFLICT",
        native_name: "CONFLICT",
    },
    ConstantInfo {
        name: "GONE",
        native_name: "GONE",
    },
    ConstantInfo {
        name: "LENGTH_REQUIRED",
        native_name: "LENGTH_REQUIRED",
    },
    ConstantInfo {
        name: "PRECONDITION_FAILED",
        native_name: "PRECONDITION_FAILED",
    },
    ConstantInfo {
        name: "PAYLOAD_TOO_LARGE",
        native_name: "PAYLOAD_TOO_LARGE",
    },
    ConstantInfo {
        name: "REQUEST_ENTITY_TOO_LARGE",
        native_name: "PAYLOAD_TOO_LARGE",
    },
    ConstantInfo {
        name: "REQUEST_URI_TOO_LONG",
        native_name: "URI_TOO_LONG",
    },
    ConstantInfo {
        name: "REQUESTED_RANGE_NOT_SATISFIABLE",
        native_name: "RANGE_NOT_SATISFIABLE",
    },
    ConstantInfo {
        name: "CONTENT_TOO_LARGE",
        native_name: "CONTENT_TOO_LARGE",
    },
    ConstantInfo {
        name: "URI_TOO_LONG",
        native_name: "URI_TOO_LONG",
    },
    ConstantInfo {
        name: "UNSUPPORTED_MEDIA_TYPE",
        native_name: "UNSUPPORTED_MEDIA_TYPE",
    },
    ConstantInfo {
        name: "RANGE_NOT_SATISFIABLE",
        native_name: "RANGE_NOT_SATISFIABLE",
    },
    ConstantInfo {
        name: "EXPECTATION_FAILED",
        native_name: "EXPECTATION_FAILED",
    },
    ConstantInfo {
        name: "IM_A_TEAPOT",
        native_name: "IM_A_TEAPOT",
    },
    ConstantInfo {
        name: "MISDIRECTED_REQUEST",
        native_name: "MISDIRECTED_REQUEST",
    },
    ConstantInfo {
        name: "UNPROCESSABLE_ENTITY",
        native_name: "UNPROCESSABLE_ENTITY",
    },
    ConstantInfo {
        name: "UNPROCESSABLE_CONTENT",
        native_name: "UNPROCESSABLE_CONTENT",
    },
    ConstantInfo {
        name: "LOCKED",
        native_name: "LOCKED",
    },
    ConstantInfo {
        name: "FAILED_DEPENDENCY",
        native_name: "FAILED_DEPENDENCY",
    },
    ConstantInfo {
        name: "TOO_EARLY",
        native_name: "TOO_EARLY",
    },
    ConstantInfo {
        name: "UPGRADE_REQUIRED",
        native_name: "UPGRADE_REQUIRED",
    },
    ConstantInfo {
        name: "PRECONDITION_REQUIRED",
        native_name: "PRECONDITION_REQUIRED",
    },
    ConstantInfo {
        name: "TOO_MANY_REQUESTS",
        native_name: "TOO_MANY_REQUESTS",
    },
    ConstantInfo {
        name: "REQUEST_HEADER_FIELDS_TOO_LARGE",
        native_name: "REQUEST_HEADER_FIELDS_TOO_LARGE",
    },
    ConstantInfo {
        name: "UNAVAILABLE_FOR_LEGAL_REASONS",
        native_name: "UNAVAILABLE_FOR_LEGAL_REASONS",
    },
    ConstantInfo {
        name: "INTERNAL_SERVER_ERROR",
        native_name: "INTERNAL_SERVER_ERROR",
    },
    ConstantInfo {
        name: "NOT_IMPLEMENTED",
        native_name: "NOT_IMPLEMENTED",
    },
    ConstantInfo {
        name: "BAD_GATEWAY",
        native_name: "BAD_GATEWAY",
    },
    ConstantInfo {
        name: "SERVICE_UNAVAILABLE",
        native_name: "SERVICE_UNAVAILABLE",
    },
    ConstantInfo {
        name: "GATEWAY_TIMEOUT",
        native_name: "GATEWAY_TIMEOUT",
    },
    ConstantInfo {
        name: "HTTP_VERSION_NOT_SUPPORTED",
        native_name: "HTTP_VERSION_NOT_SUPPORTED",
    },
    ConstantInfo {
        name: "VARIANT_ALSO_NEGOTIATES",
        native_name: "VARIANT_ALSO_NEGOTIATES",
    },
    ConstantInfo {
        name: "INSUFFICIENT_STORAGE",
        native_name: "INSUFFICIENT_STORAGE",
    },
    ConstantInfo {
        name: "LOOP_DETECTED",
        native_name: "LOOP_DETECTED",
    },
    ConstantInfo {
        name: "NOT_EXTENDED",
        native_name: "NOT_EXTENDED",
    },
    ConstantInfo {
        name: "NETWORK_AUTHENTICATION_REQUIRED",
        native_name: "NETWORK_AUTHENTICATION_REQUIRED",
    },
];
const RESTART_POLICY_CONSTANTS: &[ConstantInfo] = &[
    ConstantInfo {
        name: "TEMPORARY",
        native_name: "TEMPORARY",
    },
    ConstantInfo {
        name: "TRANSIENT",
        native_name: "TRANSIENT",
    },
    ConstantInfo {
        name: "PERMANENT",
        native_name: "PERMANENT",
    },
];
const CALL_KIND_CONSTANTS: &[ConstantInfo] = &[
    ConstantInfo {
        name: "NOT_READY",
        native_name: "NOT_READY",
    },
    ConstantInfo {
        name: "MAILBOX_FULL",
        native_name: "MAILBOX_FULL",
    },
    ConstantInfo {
        name: "MAILBOX_TIMEOUT",
        native_name: "MAILBOX_TIMEOUT",
    },
    ConstantInfo {
        name: "MESSAGE_TOO_LARGE",
        native_name: "MESSAGE_TOO_LARGE",
    },
    ConstantInfo {
        name: "REPLY_TOO_LARGE",
        native_name: "REPLY_TOO_LARGE",
    },
    ConstantInfo {
        name: "STOPPED",
        native_name: "STOPPED",
    },
    ConstantInfo {
        name: "RESTARTING",
        native_name: "RESTARTING",
    },
    ConstantInfo {
        name: "REPLY_LOST",
        native_name: "REPLY_LOST",
    },
    ConstantInfo {
        name: "REPLY_TIMEOUT",
        native_name: "REPLY_TIMEOUT",
    },
];
const WAIT_KIND_CONSTANTS: &[ConstantInfo] = &[
    ConstantInfo {
        name: "TIMEOUT",
        native_name: "TIMEOUT",
    },
    ConstantInfo {
        name: "INVALID_TIMEOUT",
        native_name: "INVALID_TIMEOUT",
    },
];
const EVENT_KIND_CONSTANTS: &[ConstantInfo] = &[
    ConstantInfo {
        name: "READY",
        native_name: "READY",
    },
    ConstantInfo {
        name: "STARTING",
        native_name: "STARTING",
    },
    ConstantInfo {
        name: "STARTED",
        native_name: "STARTED",
    },
    ConstantInfo {
        name: "FAILED",
        native_name: "FAILED",
    },
    ConstantInfo {
        name: "PANICKED",
        native_name: "PANICKED",
    },
    ConstantInfo {
        name: "RESTART_SCHEDULED",
        native_name: "RESTART_SCHEDULED",
    },
    ConstantInfo {
        name: "STOPPED",
        native_name: "STOPPED",
    },
    ConstantInfo {
        name: "INTENSITY_EXCEEDED",
        native_name: "INTENSITY_EXCEEDED",
    },
    ConstantInfo {
        name: "SHUTDOWN",
        native_name: "SHUTDOWN",
    },
    ConstantInfo {
        name: "LAGGED",
        native_name: "LAGGED",
    },
];
pub fn constants(resource: Resource) -> &'static [ConstantInfo] {
    match resource {
        Resource::Method => METHOD_CONSTANTS,
        Resource::Status => STATUS_CONSTANTS,
        Resource::RestartPolicy => RESTART_POLICY_CONSTANTS,
        Resource::CallKind => CALL_KIND_CONSTANTS,
        Resource::WaitKind => WAIT_KIND_CONSTANTS,
        Resource::EventKind => EVENT_KIND_CONSTANTS,
        _ => &[],
    }
}
pub fn constant(resource: Resource, name: &str) -> Option<ConstantInfo> {
    constants(resource)
        .iter()
        .copied()
        .find(|value| value.name == name)
}

pub fn field(resource: Resource, name: &str) -> Option<FieldInfo> {
    let view = |ty| Type::generic("view", vec![ty]);
    let (ty, owned, whole_owner, static_borrow, accessor) = match (resource, name) {
        (Resource::Request, "method") => (
            resource_type(Resource::Method, vec![]),
            true,
            false,
            false,
            "method",
        ),
        (Resource::Request, "path") => (view(Type::named("str")), false, true, false, "path"),
        (Resource::Request, "query") => (
            Type::generic("Option", vec![view(Type::named("str"))]),
            false,
            true,
            false,
            "query",
        ),
        (Resource::Request, "body") => (view(Type::named("bytes")), false, true, false, "body"),
        (Resource::Response, "status") => (
            resource_type(Resource::Status, vec![]),
            false,
            true,
            false,
            "status",
        ),
        (Resource::Response, "body") => (view(Type::named("bytes")), false, true, false, "body"),
        (Resource::Status, "value") => (Type::named("i64"), false, true, false, "value"),
        (Resource::Status, "phrase") => (view(Type::named("str")), false, true, true, "phrase"),
        (Resource::Status, "is_success") => (Type::named("bool"), false, true, false, "is_success"),
        (Resource::Status, "is_redirection") => {
            (Type::named("bool"), false, true, false, "is_redirection")
        }
        (Resource::Status, "is_client_error") => {
            (Type::named("bool"), false, true, false, "is_client_error")
        }
        (Resource::Status, "is_server_error") => {
            (Type::named("bool"), false, true, false, "is_server_error")
        }
        (Resource::Request, "is_get") => (Type::named("bool"), false, true, false, "is_get"),
        (Resource::Request, "is_post") => (Type::named("bool"), false, true, false, "is_post"),
        (Resource::Request, "is_put") => (Type::named("bool"), false, true, false, "is_put"),
        (Resource::Request, "is_delete") => (Type::named("bool"), false, true, false, "is_delete"),
        (Resource::Request, "is_patch") => (Type::named("bool"), false, true, false, "is_patch"),
        (Resource::Request, "is_head") => (Type::named("bool"), false, true, false, "is_head"),
        (Resource::Request, "is_options") => {
            (Type::named("bool"), false, true, false, "is_options")
        }
        (Resource::Request, "is_connect") => {
            (Type::named("bool"), false, true, false, "is_connect")
        }
        (Resource::Request, "is_trace") => (Type::named("bool"), false, true, false, "is_trace"),
        (Resource::WaitError, "kind") => (
            resource_type(Resource::WaitKind, vec![]),
            false,
            true,
            false,
            "kind",
        ),
        (Resource::WaitError, "message") => {
            (view(Type::named("str")), false, true, false, "message")
        }
        (Resource::CallError, "kind") => (
            resource_type(Resource::CallKind, vec![]),
            false,
            true,
            false,
            "kind",
        ),
        (Resource::CallError, "message") => {
            (view(Type::named("str")), false, true, false, "message")
        }
        (Resource::Event, "child_id") => (Type::named("i64"), false, true, false, "child_id"),
        (Resource::Event, "child_name") => {
            (view(Type::named("str")), false, true, false, "child_name")
        }
        (Resource::Event, "generation") => (Type::named("i64"), false, true, false, "generation"),
        (Resource::Event, "kind") => (
            resource_type(Resource::EventKind, vec![]),
            false,
            true,
            false,
            "kind",
        ),
        (Resource::Event, "message") => (view(Type::named("str")), false, true, false, "message"),
        (Resource::Event, "truncated") => (Type::named("bool"), false, true, false, "truncated"),
        (Resource::Event, "lost_events") => (Type::named("i64"), false, true, false, "lost_events"),
        _ => return None,
    };
    Some(FieldInfo {
        ty,
        accessor,
        owned,
        whole_owner,
        static_borrow,
    })
}
pub fn fields(resource: Resource) -> Vec<(&'static str, FieldInfo)> {
    let names: &[&str] = match resource {
        Resource::Request => &[
            "method",
            "path",
            "query",
            "body",
            "is_get",
            "is_post",
            "is_put",
            "is_delete",
            "is_patch",
            "is_head",
            "is_options",
            "is_connect",
            "is_trace",
        ],
        Resource::Response => &["status", "body"],
        Resource::CallError | Resource::WaitError => &["kind", "message"],
        Resource::Event => &[
            "child_id",
            "child_name",
            "generation",
            "kind",
            "message",
            "truncated",
            "lost_events",
        ],
        Resource::Status => &[
            "value",
            "phrase",
            "is_success",
            "is_redirection",
            "is_client_error",
            "is_server_error",
        ],
        _ => &[],
    };
    names
        .iter()
        .map(|name| (*name, field(resource, name).expect("registered field")))
        .collect()
}

pub fn declaration_source() -> String {
    module_source(&ModuleId(MODULE_ID.into()))
}
pub fn module_source(module: &ModuleId) -> String {
    if !is_registered_module(module) {
        return String::new();
    }
    let mut source =
        String::from("# Compiler-provided standard library. Imports do not start resources.\n");
    for &resource in RESOURCES {
        let info = resource_info(resource);
        if resource_module(resource) != *module {
            continue;
        }
        source.push_str(&format!(
            "resource {}{}\n",
            info.name,
            if info.type_parameters.is_empty() {
                String::new()
            } else {
                format!("[{}]", info.type_parameters.join(", "))
            }
        ));
        for (name, field) in fields(resource) {
            source.push_str(&format!("    {name}: {}\n", display_type(&field.ty)));
        }
        for constant in constants(resource) {
            source.push_str(&format!("    {}: {}\n", constant.name, info.name));
        }
        source.push('\n');
    }
    for &operation in OPERATIONS {
        let info = operation_info(operation);
        if operation_module(operation) != *module {
            continue;
        }
        source.push_str(&format!("def {}{}\n\n", info.name, info.signature));
    }
    source
}
fn display_type(ty: &Type) -> String {
    let name = resource(&ty.0)
        .map(|r| resource_info(r).name.to_owned())
        .unwrap_or_else(|| ty.0.clone());
    if ty.1.is_empty() {
        name
    } else {
        format!(
            "{name}[{}]",
            ty.1.iter().map(display_type).collect::<Vec<_>>().join(", ")
        )
    }
}
pub fn definitions(module: &ModuleId) -> Vec<DefinitionInfo> {
    if !is_registered_module(module) {
        return vec![];
    }
    let source = module_source(module);
    RESOURCES
        .iter()
        .map(|r| resource_id(*r))
        .chain(OPERATIONS.iter().map(|op| function_id(*op)))
        .filter(|id| &id.module == module)
        .map(|id| {
            let start = if id.kind == DefKind::Resource {
                format!("resource {}", id.name)
            } else {
                format!("def {}", id.name)
            };
            let line = source
                .lines()
                .position(|text| {
                    text.starts_with(&start)
                        && text[start.len()..]
                            .chars()
                            .next()
                            .is_some_and(|c| matches!(c, '[' | '(' | ' '))
                        || text == start
                })
                .unwrap_or(0)
                + 1;
            DefinitionInfo {
                symbol: crate::modules::symbol(&id),
                id,
                line,
            }
        })
        .collect()
}
pub fn member_line(resource: Resource, name: &str) -> Option<usize> {
    let mut active = false;
    for (index, line) in module_source(&resource_module(resource))
        .lines()
        .enumerate()
    {
        if line.starts_with("resource ") {
            active = line.starts_with(&format!("resource {}", resource_info(resource).name));
        }
        if active && line.starts_with(&format!("    {name}:")) {
            return Some(index + 1);
        }
    }
    None
}
