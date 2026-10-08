//! Canonical std.db.sqlite descriptors; no spelling-based resource inference.
use super::*;

static CONTRACT_POOL: ResourceContract = ResourceContract::new(
    ResourceInfo {
        module: StandardModule::Sqlite,
        name: "Pool",
        arity: 0,
        type_parameters: &[],
        inline_type_arguments: &[],
        rust_path: "::nagi_runtime::sqlite::Pool",
        copy: false,
        equality: false,
        storage: true,
        shared: true,
        debug: true,
    },
    &[],
    &[],
    &[],
    &[],
);

static CONTRACT_TX: ResourceContract = ResourceContract::new(
    ResourceInfo {
        module: StandardModule::Sqlite,
        name: "Tx",
        arity: 0,
        type_parameters: &[],
        inline_type_arguments: &[],
        rust_path: "::nagi_runtime::sqlite::Tx",
        copy: false,
        equality: false,
        storage: false,
        shared: false,
        debug: false,
    },
    &[],
    &[],
    &[],
    &[],
)
.with_lifecycle(ResourceLifecycle::SameTask);

static CONTRACT_PARAMETERS: ResourceContract = ResourceContract::new(
    ResourceInfo {
        module: StandardModule::Sqlite,
        name: "Parameters",
        arity: 0,
        type_parameters: &[],
        inline_type_arguments: &[],
        rust_path: "::nagi_runtime::sqlite::Parameters",
        copy: false,
        equality: false,
        storage: true,
        shared: false,
        debug: false,
    },
    &[],
    &[],
    &[],
    &[],
);

static CONTRACT_OPTIONS: ResourceContract = ResourceContract::new(
    ResourceInfo {
        module: StandardModule::Sqlite,
        name: "Options",
        arity: 0,
        type_parameters: &[],
        inline_type_arguments: &[],
        rust_path: "::nagi_runtime::sqlite::Options",
        copy: false,
        equality: false,
        storage: true,
        shared: false,
        debug: true,
    },
    &[],
    &[],
    &[],
    &[],
);

static CONTRACT_BEGINMODE: ResourceContract = ResourceContract::new(
    ResourceInfo {
        module: StandardModule::Sqlite,
        name: "BeginMode",
        arity: 0,
        type_parameters: &[],
        inline_type_arguments: &[],
        rust_path: "::nagi_runtime::sqlite::BeginMode",
        copy: true,
        equality: true,
        storage: true,
        shared: true,
        debug: true,
    },
    &[],
    &[],
    &[],
    &[],
);

static CONTRACT_FAILURE: ResourceContract = ResourceContract::new(
    ResourceInfo {
        module: StandardModule::Sqlite,
        name: "Failure",
        arity: 0,
        type_parameters: &[],
        inline_type_arguments: &[],
        rust_path: "::nagi_runtime::sqlite::Failure",
        copy: false,
        equality: false,
        storage: true,
        shared: true,
        debug: true,
    },
    &[],
    &[],
    &[],
    &[],
);

static CONTRACT_FAILUREKIND: ResourceContract = ResourceContract::new(
    ResourceInfo {
        module: StandardModule::Sqlite,
        name: "FailureKind",
        arity: 0,
        type_parameters: &[],
        inline_type_arguments: &[],
        rust_path: "::nagi_runtime::sqlite::FailureKind",
        copy: true,
        equality: true,
        storage: true,
        shared: true,
        debug: true,
    },
    &[],
    &[],
    &[],
    &[],
);

static CONTRACT_OUTCOME: ResourceContract = ResourceContract::new(
    ResourceInfo {
        module: StandardModule::Sqlite,
        name: "Outcome",
        arity: 0,
        type_parameters: &[],
        inline_type_arguments: &[],
        rust_path: "::nagi_runtime::sqlite::Outcome",
        copy: true,
        equality: true,
        storage: true,
        shared: true,
        debug: true,
    },
    &[],
    &[],
    &[],
    &[],
);

pub(super) fn resource_contract(resource: Resource) -> &'static ResourceContract {
    match resource {
        Resource::SqlitePool => &CONTRACT_POOL,
        Resource::SqliteTx => &CONTRACT_TX,
        Resource::SqliteParameters => &CONTRACT_PARAMETERS,
        Resource::SqliteOptions => &CONTRACT_OPTIONS,
        Resource::SqliteBeginMode => &CONTRACT_BEGINMODE,
        Resource::SqliteFailure => &CONTRACT_FAILURE,
        Resource::SqliteFailureKind => &CONTRACT_FAILUREKIND,
        Resource::SqliteOutcome => &CONTRACT_OUTCOME,
        _ => unreachable!("not a SQLite resource"),
    }
}

pub(super) fn operation_info(operation: Operation) -> &'static OperationInfo {
    match operation {
 Operation::SqliteOptions => &OperationInfo { module: StandardModule::Sqlite,
 name: "options", rust_path: "::nagi_runtime::sqlite::options", arity: 4, generic_arity: 0,
 type_parameters: &[], asynchronous: false, emit_type_arguments: false,
 parameters: &[Passing::Move, Passing::Move, Passing::Move, Passing::Move], borrow_owner: None,
 signature: "(connections: i64, queue_capacity: i64, acquire_ms: i64, busy_ms: i64) -> Result[Options, Error]" },
 Operation::SqliteOpen => &OperationInfo { module: StandardModule::Sqlite,
 name: "open", rust_path: "::nagi_runtime::sqlite::open", arity: 2, generic_arity: 0,
 type_parameters: &[], asynchronous: true, emit_type_arguments: false,
 parameters: &[Passing::Reference, Passing::Move], borrow_owner: None,
 signature: "(path: view[str], options: Options) -> Future[Result[Pool, Failure]]" },
 Operation::SqliteClonePool => &OperationInfo { module: StandardModule::Sqlite,
 name: "clone_pool", rust_path: "::nagi_runtime::sqlite::clone_pool", arity: 1, generic_arity: 0,
 type_parameters: &[], asynchronous: false, emit_type_arguments: false,
 parameters: &[Passing::Reference], borrow_owner: None,
 signature: "(pool: view[Pool]) -> Pool" },
 Operation::SqliteBegin => &OperationInfo { module: StandardModule::Sqlite,
 name: "begin", rust_path: "::nagi_runtime::sqlite::begin", arity: 2, generic_arity: 0,
 type_parameters: &[], asynchronous: true, emit_type_arguments: false,
 parameters: &[Passing::Reference, Passing::Move], borrow_owner: None,
 signature: "(pool: view[Pool], mode: BeginMode) -> Future[Result[Tx, Failure]]" },
 Operation::SqliteParameters => &OperationInfo { module: StandardModule::Sqlite,
 name: "parameters", rust_path: "::nagi_runtime::sqlite::parameters", arity: 0, generic_arity: 0,
 type_parameters: &[], asynchronous: false, emit_type_arguments: false,
 parameters: &[], borrow_owner: None,
 signature: "() -> Parameters" },
 Operation::SqliteBindI64 => &OperationInfo { module: StandardModule::Sqlite,
 name: "bind_i64", rust_path: "::nagi_runtime::sqlite::bind_i64", arity: 2, generic_arity: 0,
 type_parameters: &[], asynchronous: false, emit_type_arguments: false,
 parameters: &[Passing::Move, Passing::Move], borrow_owner: None,
 signature: "(parameters: Parameters, value: i64) -> Parameters" },
 Operation::SqliteBindF64 => &OperationInfo { module: StandardModule::Sqlite,
 name: "bind_f64", rust_path: "::nagi_runtime::sqlite::bind_f64", arity: 2, generic_arity: 0,
 type_parameters: &[], asynchronous: false, emit_type_arguments: false,
 parameters: &[Passing::Move, Passing::Move], borrow_owner: None,
 signature: "(parameters: Parameters, value: f64) -> Result[Parameters, Error]" },
 Operation::SqliteBindText => &OperationInfo { module: StandardModule::Sqlite,
 name: "bind_text", rust_path: "::nagi_runtime::sqlite::bind_text", arity: 2, generic_arity: 0,
 type_parameters: &[], asynchronous: false, emit_type_arguments: false,
 parameters: &[Passing::Move, Passing::Move], borrow_owner: None,
 signature: "(parameters: Parameters, value: str) -> Parameters" },
 Operation::SqliteBindBytes => &OperationInfo { module: StandardModule::Sqlite,
 name: "bind_bytes", rust_path: "::nagi_runtime::sqlite::bind_bytes", arity: 2, generic_arity: 0,
 type_parameters: &[], asynchronous: false, emit_type_arguments: false,
 parameters: &[Passing::Move, Passing::Move], borrow_owner: None,
 signature: "(parameters: Parameters, value: bytes) -> Parameters" },
 Operation::SqliteBindNull => &OperationInfo { module: StandardModule::Sqlite,
 name: "bind_null", rust_path: "::nagi_runtime::sqlite::bind_null", arity: 1, generic_arity: 0,
 type_parameters: &[], asynchronous: false, emit_type_arguments: false,
 parameters: &[Passing::Move], borrow_owner: None,
 signature: "(parameters: Parameters) -> Parameters" },
 Operation::SqliteQuery => &OperationInfo { module: StandardModule::Sqlite,
 name: "query", rust_path: "::nagi_runtime::sqlite::query", arity: 3, generic_arity: 1,
 type_parameters: &["T"], asynchronous: true, emit_type_arguments: true,
 parameters: &[Passing::Reference, Passing::Reference, Passing::Move], borrow_owner: None,
 signature: "[T](tx: view[Tx], sql: view[str], parameters: Parameters) -> Future[Result[Option[T], Failure]]" },
 Operation::SqliteAll => &OperationInfo { module: StandardModule::Sqlite,
 name: "all", rust_path: "::nagi_runtime::sqlite::all", arity: 3, generic_arity: 1,
 type_parameters: &["T"], asynchronous: true, emit_type_arguments: true,
 parameters: &[Passing::Reference, Passing::Reference, Passing::Move], borrow_owner: None,
 signature: "[T](tx: view[Tx], sql: view[str], parameters: Parameters) -> Future[Result[List[T], Failure]]" },
 Operation::SqliteExec => &OperationInfo { module: StandardModule::Sqlite,
 name: "exec", rust_path: "::nagi_runtime::sqlite::exec", arity: 3, generic_arity: 0,
 type_parameters: &[], asynchronous: true, emit_type_arguments: false,
 parameters: &[Passing::Reference, Passing::Reference, Passing::Move], borrow_owner: None,
 signature: "(tx: view[Tx], sql: view[str], parameters: Parameters) -> Future[Result[i64, Failure]]" },
 Operation::SqliteCommit => &OperationInfo { module: StandardModule::Sqlite,
 name: "commit", rust_path: "::nagi_runtime::sqlite::commit", arity: 1, generic_arity: 0,
 type_parameters: &[], asynchronous: true, emit_type_arguments: false,
 parameters: &[Passing::Move], borrow_owner: None,
 signature: "(tx: Tx) -> Future[Result[unit, Failure]]" },
 Operation::SqliteRollback => &OperationInfo { module: StandardModule::Sqlite,
 name: "rollback", rust_path: "::nagi_runtime::sqlite::rollback", arity: 1, generic_arity: 0,
 type_parameters: &[], asynchronous: true, emit_type_arguments: false,
 parameters: &[Passing::Move], borrow_owner: None,
 signature: "(tx: Tx) -> Future[Result[unit, Failure]]" },
 Operation::SqliteClose => &OperationInfo { module: StandardModule::Sqlite,
 name: "close", rust_path: "::nagi_runtime::sqlite::close", arity: 2, generic_arity: 0,
 type_parameters: &[], asynchronous: true, emit_type_arguments: false,
 parameters: &[Passing::Reference, Passing::Move], borrow_owner: None,
 signature: "(pool: view[Pool], timeout_ms: i64) -> Future[Result[unit, Failure]]" },
 Operation::SqliteCopyPrimaryError => &OperationInfo { module: StandardModule::Sqlite,
 name: "copy_primary_error", rust_path: "::nagi_runtime::sqlite::copy_primary_error", arity: 1, generic_arity: 0,
 type_parameters: &[], asynchronous: false, emit_type_arguments: false,
 parameters: &[Passing::Reference], borrow_owner: None,
 signature: "(problem: view[Failure]) -> Option[Error]" },
 Operation::SqliteCopyCleanupError => &OperationInfo { module: StandardModule::Sqlite,
 name: "copy_cleanup_error", rust_path: "::nagi_runtime::sqlite::copy_cleanup_error", arity: 1, generic_arity: 0,
 type_parameters: &[], asynchronous: false, emit_type_arguments: false,
 parameters: &[Passing::Reference], borrow_owner: None,
 signature: "(problem: view[Failure]) -> Option[Error]" },
 _ => unreachable!("not a SQLite operation"),
 }
}

pub(super) fn constants(resource: Resource) -> &'static [ConstantInfo] {
    match resource {
        Resource::SqliteBeginMode => &[
            ConstantInfo {
                name: "DEFERRED",
                native_name: "Deferred",
            },
            ConstantInfo {
                name: "IMMEDIATE",
                native_name: "Immediate",
            },
            ConstantInfo {
                name: "EXCLUSIVE",
                native_name: "Exclusive",
            },
        ],
        Resource::SqliteFailureKind => &[
            ConstantInfo {
                name: "INVALID",
                native_name: "Invalid",
            },
            ConstantInfo {
                name: "CLOSED",
                native_name: "Closed",
            },
            ConstantInfo {
                name: "ACQUIRE_TIMEOUT",
                native_name: "AcquireTimeout",
            },
            ConstantInfo {
                name: "BUSY",
                native_name: "Busy",
            },
            ConstantInfo {
                name: "SQL",
                native_name: "Sql",
            },
            ConstantInfo {
                name: "BIND",
                native_name: "Bind",
            },
            ConstantInfo {
                name: "DECODE",
                native_name: "Decode",
            },
            ConstantInfo {
                name: "ABORTED",
                native_name: "Aborted",
            },
            ConstantInfo {
                name: "CLEANUP",
                native_name: "Cleanup",
            },
            ConstantInfo {
                name: "WORKER",
                native_name: "Worker",
            },
            ConstantInfo {
                name: "REPLY_LOST",
                native_name: "ReplyLost",
            },
            ConstantInfo {
                name: "CLOSE_TIMEOUT",
                native_name: "CloseTimeout",
            },
            ConstantInfo {
                name: "ALLOCATION",
                native_name: "Allocation",
            },
        ],
        Resource::SqliteOutcome => &[
            ConstantInfo {
                name: "NOT_APPLICABLE",
                native_name: "NotApplicable",
            },
            ConstantInfo {
                name: "ACTIVE",
                native_name: "Active",
            },
            ConstantInfo {
                name: "COMMITTED",
                native_name: "Committed",
            },
            ConstantInfo {
                name: "ROLLED_BACK",
                native_name: "RolledBack",
            },
            ConstantInfo {
                name: "UNKNOWN",
                native_name: "Unknown",
            },
        ],
        _ => &[],
    }
}
