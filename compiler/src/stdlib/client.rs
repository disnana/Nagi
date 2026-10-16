//! Adopted SF06 resources. Registration alone is not native acceptance.
use super::*;

const fn owned(
    name: &'static str,
    rust_path: &'static str,
    copy: bool,
    debug: bool,
) -> ResourceContract {
    ResourceContract::new(
        ResourceInfo {
            module: StandardModule::HttpClient,
            name,
            arity: 0,
            type_parameters: &[],
            inline_type_arguments: &[],
            rust_path,
            copy,
            equality: copy,
            storage: true,
            shared: true,
            debug,
        },
        &[],
        &[],
        &[],
        &[],
    )
}

static PROTECTED: ResourceContract = ResourceContract::new(
    ResourceInfo {
        module: StandardModule::HttpClient,
        name: "ProtectedOperation",
        arity: 3,
        type_parameters: &["C", "P", "I"],
        inline_type_arguments: &[],
        rust_path: "::nagi_runtime::http_client::ProtectedOperation",
        copy: false,
        equality: false,
        storage: true,
        shared: true,
        debug: true,
    },
    &[0],
    &[2],
    &[],
    &[1],
);
static PUBLIC: ResourceContract = ResourceContract::new(
    ResourceInfo {
        module: StandardModule::HttpClient,
        name: "PublicOperation",
        arity: 2,
        type_parameters: &["C", "I"],
        inline_type_arguments: &[],
        rust_path: "::nagi_runtime::http_client::PublicOperation",
        copy: false,
        equality: false,
        storage: true,
        shared: true,
        debug: true,
    },
    &[0],
    &[1],
    &[],
    &[],
);
static RESPONSE: ResourceContract = owned(
    "Response",
    "::nagi_runtime::http_client::Response",
    false,
    false,
);
static FAILURE: ResourceContract = owned(
    "Failure",
    "::nagi_runtime::http_client::Failure",
    false,
    true,
);
static KIND: ResourceContract = owned(
    "FailureKind",
    "::nagi_runtime::http_client::FailureKind",
    true,
    true,
);
static OUTCOME: ResourceContract = owned(
    "Outcome",
    "::nagi_runtime::http_client::Outcome",
    true,
    true,
);

pub(super) fn resource_contract(resource: Resource) -> &'static ResourceContract {
    match resource {
        Resource::ProtectedOperation => &PROTECTED,
        Resource::PublicOperation => &PUBLIC,
        Resource::ClientResponse => &RESPONSE,
        Resource::ClientFailure => &FAILURE,
        Resource::ClientFailureKind => &KIND,
        Resource::ClientOutcome => &OUTCOME,
        _ => unreachable!(),
    }
}

macro_rules! op {
    ($name:literal, $path:literal, $types:expr, $passing:expr, $borrow:expr, $async:expr, $signature:literal) => {{
        static INFO: OperationInfo = OperationInfo {
            module: StandardModule::HttpClient,
            name: $name,
            rust_path: $path,
            arity: ($passing as &[Passing]).len(),
            generic_arity: ($types as &[&str]).len(),
            type_parameters: $types,
            asynchronous: $async,
            emit_type_arguments: true,
            parameters: $passing,
            borrow_owner: $borrow,
            signature: $signature,
        };
        &INFO
    }};
}

pub(super) fn operation_info(operation: Operation) -> &'static OperationInfo {
    use Passing::{Move, Reference};
    match operation {
        Operation::ClientExecute => op!("execute", "::nagi_runtime::http_client::execute", &["C", "P", "I"], &[Reference, Move, Move], None, true, "[C, P, I](operation: view[ProtectedOperation[C, P, I]], grant: Grant[P], input: I) -> Future[Result[Response, Failure]]"),
        Operation::ClientExecutePublic => op!("execute_public", "::nagi_runtime::http_client::execute_public", &["C", "I"], &[Reference, Move], None, true, "[C, I](operation: view[PublicOperation[C, I]], input: I) -> Future[Result[Response, Failure]]"),
        Operation::ClientStatus => op!("status", "::nagi_runtime::http_client::status", &[], &[Reference], None, false, "(response: view[Response]) -> i64"),
        Operation::ClientHeaders => op!("headers", "::nagi_runtime::http_client::headers", &[], &[Reference, Reference], Some(0), false, "(response: view[Response], name: view[str]) -> Result[List[view[bytes]], Error]"),
        Operation::ClientBody => op!("body", "::nagi_runtime::http_client::body", &[], &[Reference], Some(0), false, "(response: view[Response]) -> view[bytes]"),
        Operation::ClientTakeBody => op!("take_body", "::nagi_runtime::http_client::take_body", &[], &[Move], None, false, "(response: Response) -> bytes"),
        Operation::ClientKind => op!("kind", "::nagi_runtime::http_client::kind", &[], &[Reference], None, false, "(failure: view[Failure]) -> FailureKind"),
        Operation::ClientMessage => op!("message", "::nagi_runtime::http_client::message", &[], &[Reference], Some(0), false, "(failure: view[Failure]) -> view[str]"),
        Operation::ClientAuthKind => op!("auth_kind", "::nagi_runtime::http_client::auth_kind", &[], &[Reference], None, false, "(failure: view[Failure]) -> Option[auth.FailureKind]"),
        Operation::ClientOutcome => op!("outcome", "::nagi_runtime::http_client::outcome", &[], &[Reference], None, false, "(failure: view[Failure]) -> Outcome"),
        _ => unreachable!(),
    }
}

const FAILURE_CONSTANTS: &[ConstantInfo] = &[
    ConstantInfo {
        name: "INVALID_INPUT",
        native_name: "InvalidInput",
    },
    ConstantInfo {
        name: "SECURITY",
        native_name: "Security",
    },
    ConstantInfo {
        name: "DESTINATION_DENIED",
        native_name: "DestinationDenied",
    },
    ConstantInfo {
        name: "CAPACITY",
        native_name: "Capacity",
    },
    ConstantInfo {
        name: "CLOSING",
        native_name: "Closing",
    },
    ConstantInfo {
        name: "DEADLINE",
        native_name: "Deadline",
    },
    ConstantInfo {
        name: "DNS",
        native_name: "Dns",
    },
    ConstantInfo {
        name: "TCP",
        native_name: "Tcp",
    },
    ConstantInfo {
        name: "TLS",
        native_name: "Tls",
    },
    ConstantInfo {
        name: "PROTOCOL",
        native_name: "Protocol",
    },
    ConstantInfo {
        name: "RESPONSE_LIMIT",
        native_name: "ResponseLimit",
    },
    ConstantInfo {
        name: "UNSUPPORTED_ENCODING",
        native_name: "UnsupportedEncoding",
    },
    ConstantInfo {
        name: "UNAVAILABLE",
        native_name: "Unavailable",
    },
    ConstantInfo {
        name: "INTERNAL",
        native_name: "Internal",
    },
];
const OUTCOME_CONSTANTS: &[ConstantInfo] = &[
    ConstantInfo {
        name: "REQUEST_NOT_STARTED",
        native_name: "RequestNotStarted",
    },
    ConstantInfo {
        name: "UNKNOWN",
        native_name: "Unknown",
    },
];
pub(super) fn constants(resource: Resource) -> &'static [ConstantInfo] {
    match resource {
        Resource::ClientFailureKind => FAILURE_CONSTANTS,
        Resource::ClientOutcome => OUTCOME_CONSTANTS,
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    #[test]
    fn operation_roles_keep_physical_context_distinct_from_permission_and_input() {
        use TypeArgumentRole::*;
        assert_eq!(PROTECTED.positions(SharedPayload), &[0]);
        assert_eq!(PROTECTED.positions(NominalPhantom), &[1]);
        assert_eq!(PROTECTED.positions(IndirectProtocol), &[2]);
        assert!(PROTECTED.positions(InlinePayload).is_empty());
        assert!(PROTECTED.positions(CallbackSignature).is_empty());
        assert_eq!(PUBLIC.positions(SharedPayload), &[0]);
        assert_eq!(PUBLIC.positions(IndirectProtocol), &[1]);
        assert!(PUBLIC.positions(NominalPhantom).is_empty());
        assert!(!native_serde_supported(Resource::ProtectedOperation));
        assert!(!native_serde_supported(Resource::PublicOperation));
    }
}
