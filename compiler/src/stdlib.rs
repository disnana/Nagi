//! Compiler-owned standard definitions. Serialized Low names this registry;
//! it cannot choose native paths, capabilities, or callback contracts.
use crate::ast::*;

pub const MODULE_NAME: &str = "std.http.server";
pub const MODULE_ID: &str = "stdlib:std.http.server";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Resource {
    Request,
    Response,
    Method,
    Status,
    Options,
    App,
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
    pub name: &'static str,
    pub arity: usize,
    pub rust_path: &'static str,
    pub copy: bool,
    pub equality: bool,
    pub storage: bool,
    pub shared: bool,
    pub debug: bool,
}
#[derive(Debug)]
pub struct OperationInfo {
    pub name: &'static str,
    pub rust_path: &'static str,
    pub arity: usize,
    pub generic_arity: usize,
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
];

pub fn module(name: &str) -> Option<ModuleId> {
    (name == MODULE_NAME).then(|| ModuleId(MODULE_ID.into()))
}
pub fn is_registered_module(id: &ModuleId) -> bool {
    id.0 == MODULE_ID
}
pub fn resource_info(resource: Resource) -> &'static ResourceInfo {
    match resource {
        Resource::Request => &ResourceInfo {
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
            name: "App",
            arity: 2,
            rust_path: "::nagi_runtime::http_server::App",
            copy: false,
            equality: false,
            storage: true,
            shared: false,
            debug: true,
        },
    }
}
pub fn operation_info(operation: Operation) -> &'static OperationInfo {
    match operation {
 Operation::Status => &OperationInfo { name: "status", rust_path: "::nagi_runtime::http_server::status", arity: 1, generic_arity: 0, parameters: &[Passing::Move], borrow_owner: None, signature: "(value: i64) -> Result[Status, Error]" },
 Operation::Method => &OperationInfo { name: "method", rust_path: "::nagi_runtime::http_server::method", arity: 1, generic_arity: 0, parameters: &[Passing::Reference], borrow_owner: None, signature: "(name: view[str]) -> Result[Method, Error]" },
 Operation::MethodName => &OperationInfo { name: "method_name", rust_path: "::nagi_runtime::http_server::method_name", arity: 1, generic_arity: 0, parameters: &[Passing::Reference], borrow_owner: Some(0), signature: "(method: view[Method]) -> view[str]" },
 Operation::Empty => &OperationInfo { name: "empty", rust_path: "::nagi_runtime::http_server::empty", arity: 1, generic_arity: 0, parameters: &[Passing::Move], borrow_owner: None, signature: "(status: Status) -> Response" },
 Operation::Text => &OperationInfo { name: "text", rust_path: "::nagi_runtime::http_server::text", arity: 2, generic_arity: 0, parameters: &[Passing::Move, Passing::Reference], borrow_owner: None, signature: "(status: Status, body: view[str]) -> Response" },
 Operation::Html => &OperationInfo { name: "html", rust_path: "::nagi_runtime::http_server::html", arity: 2, generic_arity: 0, parameters: &[Passing::Move, Passing::Reference], borrow_owner: None, signature: "(status: Status, body: view[str]) -> Response" },
 Operation::Bytes => &OperationInfo { name: "bytes", rust_path: "::nagi_runtime::http_server::bytes", arity: 2, generic_arity: 0, parameters: &[Passing::Move, Passing::Reference], borrow_owner: None, signature: "(status: Status, body: view[bytes]) -> Response" },
 Operation::Json => &OperationInfo { name: "json", rust_path: "::nagi_runtime::http_server::json", arity: 2, generic_arity: 1, parameters: &[Passing::Move, Passing::Borrow], borrow_owner: None, signature: "[T](status: Status, value: T) -> Result[Response, Error]" },
 Operation::AppendHeader => &OperationInfo { name: "append_header", rust_path: "::nagi_runtime::http_server::append_header", arity: 3, generic_arity: 0, parameters: &[Passing::Move, Passing::Reference, Passing::Reference], borrow_owner: None, signature: "(response: Response, name: view[str], value: view[bytes]) -> Result[Response, Error]" },
 Operation::AppendHeaderText => &OperationInfo { name: "append_header_text", rust_path: "::nagi_runtime::http_server::append_header_text", arity: 3, generic_arity: 0, parameters: &[Passing::Move, Passing::Reference, Passing::Reference], borrow_owner: None, signature: "(response: Response, name: view[str], value: view[str]) -> Result[Response, Error]" },
 Operation::Header => &OperationInfo { name: "header", rust_path: "::nagi_runtime::http_server::header", arity: 2, generic_arity: 0, parameters: &[Passing::Reference, Passing::Reference], borrow_owner: Some(0), signature: "(request: view[Request], name: view[str]) -> Result[Option[view[bytes]], Error]" },
 Operation::HeaderText => &OperationInfo { name: "header_text", rust_path: "::nagi_runtime::http_server::header_text", arity: 2, generic_arity: 0, parameters: &[Passing::Reference, Passing::Reference], borrow_owner: Some(0), signature: "(request: view[Request], name: view[str]) -> Result[Option[view[str]], Error]" },
 Operation::Headers => &OperationInfo { name: "headers", rust_path: "::nagi_runtime::http_server::headers", arity: 2, generic_arity: 0, parameters: &[Passing::Reference, Passing::Reference], borrow_owner: Some(0), signature: "(request: view[Request], name: view[str]) -> Result[List[view[bytes]], Error]" },
 Operation::DefaultOptions => &OperationInfo { name: "default_options", rust_path: "::nagi_runtime::http_server::default_options", arity: 0, generic_arity: 0, parameters: &[], borrow_owner: None, signature: "() -> Options" },
 Operation::Options => &OperationInfo { name: "options", rust_path: "::nagi_runtime::http_server::options", arity: 4, generic_arity: 0, parameters: &[Passing::Move, Passing::Move, Passing::Move, Passing::Move], borrow_owner: None, signature: "(body_bytes: i64, body_ms: i64, handler_ms: i64, shutdown_ms: i64) -> Result[Options, Error]" },
 Operation::Capacity => &OperationInfo { name: "capacity", rust_path: "::nagi_runtime::http_server::capacity", arity: 3, generic_arity: 0, parameters: &[Passing::Move, Passing::Move, Passing::Move], borrow_owner: None, signature: "(options: Options, connections: i64, requests: i64) -> Result[Options, Error]" },
 Operation::HeaderTimeout => &OperationInfo { name: "header_timeout", rust_path: "::nagi_runtime::http_server::header_timeout", arity: 2, generic_arity: 0, parameters: &[Passing::Move, Passing::Move], borrow_owner: None, signature: "(options: Options, milliseconds: i64) -> Result[Options, Error]" },
 Operation::HeaderLimits => &OperationInfo { name: "header_limits", rust_path: "::nagi_runtime::http_server::header_limits", arity: 3, generic_arity: 0, parameters: &[Passing::Move, Passing::Move, Passing::Move], borrow_owner: None, signature: "(options: Options, bytes: i64, count: i64) -> Result[Options, Error]" },
 Operation::SendTimeout => &OperationInfo { name: "send_timeout", rust_path: "::nagi_runtime::http_server::send_timeout", arity: 2, generic_arity: 0, parameters: &[Passing::Move, Passing::Move], borrow_owner: None, signature: "(options: Options, milliseconds: i64) -> Result[Options, Error]" },
 Operation::App => &OperationInfo { name: "app", rust_path: "::nagi_runtime::http_server::app", arity: 2, generic_arity: 2, parameters: &[Passing::Move, Passing::Mapper], borrow_owner: None, signature: "[S, E](state: S, mapper: fn[E, Response]) -> App[S, E]" },
 Operation::AppDefault => &OperationInfo { name: "app_default", rust_path: "::nagi_runtime::http_server::app_default", arity: 1, generic_arity: 1, parameters: &[Passing::Move], borrow_owner: None, signature: "[S](state: S) -> App[S, Error]" },
 Operation::Route => &OperationInfo { name: "route", rust_path: "::nagi_runtime::http_server::route", arity: 4, generic_arity: 0, parameters: &[Passing::Move, Passing::Move, Passing::Reference, Passing::Handler], borrow_owner: None, signature: "(app: App[S, E], method: Method, path: view[str], handler: fn[Request, shared[S], Future[Result[Response, E]]]) -> Result[App[S, E], Error]" },
 Operation::RouteMapped => &OperationInfo { name: "route_mapped", rust_path: "::nagi_runtime::http_server::route_mapped", arity: 5, generic_arity: 0, parameters: &[Passing::Move, Passing::Move, Passing::Reference, Passing::Handler, Passing::Mapper], borrow_owner: None, signature: "(app: App[S, E], method: Method, path: view[str], handler: fn[Request, shared[S], Future[Result[Response, F]]], mapper: fn[F, Response]) -> Result[App[S, E], Error]" },
 Operation::Serve => &OperationInfo { name: "serve", rust_path: "::nagi_runtime::http_server::serve", arity: 3, generic_arity: 0, parameters: &[Passing::Move, Passing::Move, Passing::Move], borrow_owner: None, signature: "(app: App[S, E], port: i64, options: Options) -> Future[Result[unit, Error]]" },
}
}
pub fn resource_id(resource: Resource) -> DefId {
    DefId {
        module: ModuleId(MODULE_ID.into()),
        kind: DefKind::Resource,
        name: resource_info(resource).name.into(),
    }
}
pub fn function_id(operation: Operation) -> DefId {
    DefId {
        module: ModuleId(MODULE_ID.into()),
        kind: DefKind::Function,
        name: operation_info(operation).name.into(),
    }
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
pub fn constants(resource: Resource) -> &'static [ConstantInfo] {
    match resource {
        Resource::Method => METHOD_CONSTANTS,
        Resource::Status => STATUS_CONSTANTS,
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
    let mut source =
        String::from("# Compiler-provided standard library. Imports do not start resources.\n");
    for &resource in RESOURCES {
        let info = resource_info(resource);
        source.push_str(&format!(
            "resource {}{}\n",
            info.name,
            if info.arity == 2 { "[S, E]" } else { "" }
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
    let source = declaration_source();
    RESOURCES
        .iter()
        .map(|r| resource_id(*r))
        .chain(OPERATIONS.iter().map(|op| function_id(*op)))
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
    for (index, line) in declaration_source().lines().enumerate() {
        if line.starts_with("resource ") {
            active = line.starts_with(&format!("resource {}", resource_info(resource).name));
        }
        if active && line.starts_with(&format!("    {name}:")) {
            return Some(index + 1);
        }
    }
    None
}
