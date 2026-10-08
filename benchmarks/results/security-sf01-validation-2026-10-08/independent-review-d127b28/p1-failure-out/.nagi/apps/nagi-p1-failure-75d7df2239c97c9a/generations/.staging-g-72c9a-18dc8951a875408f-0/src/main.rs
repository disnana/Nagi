#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_main() -> () {
    let mut values: ::std::vec::Vec<::std::option::Option<::nagi_runtime::auth::Failure>> = vec![::std::option::Option::Some(::nagi_runtime::auth::denied())];
    let mut duplicate: ::std::vec::Vec<::std::option::Option<::nagi_runtime::auth::Failure>> = ((values).as_slice()).to_vec();
    println!("{}", ((duplicate).len() as ::std::primitive::i64));
}
#[allow(non_snake_case)]
pub mod auth {
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::AuthScope as AuthScope;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::VerifiedIdentity as VerifiedIdentity;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::Failure as Failure;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::FailureKind as FailureKind;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::Grant as Grant;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::subject as subject;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::kind as kind;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::message as message;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::invalid_credential as invalid_credential;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::denied as denied;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::expired as expired;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::invalid_request as invalid_request;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::unavailable as unavailable;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::internal as internal;
}
fn main() {
__nagi_main();
}
