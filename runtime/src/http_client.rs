//! Fixed-service typed outbound operations. Host startup is trusted native code.
pub(crate) mod address;
pub(crate) mod ledger;
pub use ledger::operation::{
    auth_kind, body, execute, execute_public, headers, kind, message, outcome, status, take_body,
    Failure, FailureKind, Outcome, ProtectedOperation, PublicOperation, Response,
};
/// Native development host plumbing; no Nagi constructors/defaults are registered.
#[doc(hidden)]
pub mod startup {
    pub use super::ledger::operation::startup::{
        register_protected, register_public, FixedRequestHeaders, OperationLimits, Owner,
        ProtectedSpec, PublicSpec, ServiceConfig, ServiceCredential, ServiceKey, ServiceSpec,
        StartupCatalog,
    };
    pub use super::ledger::operation::{
        BoundValues, FrozenInput, InputLimits, PreparedRequest, RequestLimits,
    };
}

#[cfg(test)]
#[path = "http_client/startup_tests.rs"]
pub(crate) mod startup_tests;
