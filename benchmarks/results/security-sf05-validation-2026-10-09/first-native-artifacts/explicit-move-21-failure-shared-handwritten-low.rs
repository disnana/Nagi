#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
pub struct __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31352f68616e647772697474656e2e6c6f77_c_486f6c646572 {
    pub failure: ::nagi_runtime::sqlite::Failure,
}
impl ::std::fmt::Debug for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31352f68616e647772697474656e2e6c6f77_c_486f6c646572 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Holder").field("failure", &self.failure).finish()
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31352f68616e647772697474656e2e6c6f77_f_7075626c697368(mut problem: ::nagi_runtime::sqlite::Failure) -> ::std::sync::Arc<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31352f68616e647772697474656e2e6c6f77_c_486f6c646572> {
    return ::std::sync::Arc::new(__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31352f68616e647772697474656e2e6c6f77_c_486f6c646572 { failure: problem });
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31352f68616e647772697474656e2e6c6f77_f_7075626c6973685f646972656374(mut problem: ::nagi_runtime::sqlite::Failure) -> ::std::sync::Arc<::nagi_runtime::sqlite::Failure> {
    return ::std::sync::Arc::new(problem);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31352f68616e647772697474656e2e6c6f77_f_7075626c6973685f6f7074696f6e(mut problem: ::nagi_runtime::sqlite::Failure) -> ::std::sync::Arc<::std::option::Option<::nagi_runtime::sqlite::Failure>> {
    return ::std::sync::Arc::new(::std::option::Option::Some(problem));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31352f68616e647772697474656e2e6c6f77_f_7075626c6973685f726573756c74(mut problem: ::nagi_runtime::sqlite::Failure) -> ::std::sync::Arc<::std::result::Result<::std::primitive::i64, ::nagi_runtime::sqlite::Failure>> {
    let mut wrapped: ::std::result::Result<::std::primitive::i64, ::nagi_runtime::sqlite::Failure> = ::std::result::Result::Err(problem);
    return ::std::sync::Arc::new(wrapped);
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31352f68616e647772697474656e2e6c6f77_c_486f6c646572 as Holder;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31352f68616e647772697474656e2e6c6f77_f_7075626c697368 as publish;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31352f68616e647772697474656e2e6c6f77_f_7075626c6973685f646972656374 as publish_direct;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31352f68616e647772697474656e2e6c6f77_f_7075626c6973685f6f7074696f6e as publish_option;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31352f68616e647772697474656e2e6c6f77_f_7075626c6973685f726573756c74 as publish_result;
#[allow(non_snake_case)]
pub mod sqlite {
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::Pool as Pool;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::Tx as Tx;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::Query as Query;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::Parameters as Parameters;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::Options as Options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::BeginMode as BeginMode;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::Failure as Failure;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::FailureKind as FailureKind;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::Outcome as Outcome;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::literal as literal;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::options as options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::open as open;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::clone_pool as clone_pool;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::begin as begin;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::parameters as parameters;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::bind_i64 as bind_i64;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::bind_f64 as bind_f64;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::bind_text as bind_text;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::bind_bytes as bind_bytes;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::bind_null as bind_null;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::query as query;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::all as all;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::exec as exec;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::commit as commit;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::rollback as rollback;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::close as close;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::copy_primary_error as copy_primary_error;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::copy_cleanup_error as copy_cleanup_error;
}
fn main() {}



fn problem() -> nagi_runtime::sqlite::Failure {
    let mut failure = tokio::runtime::Runtime::new().unwrap().block_on(nagi_runtime::sqlite::open("",nagi_runtime::sqlite::options(1,2,1000,0).unwrap())).unwrap_err();
    failure.message = "PRIVATE_FAILURE_MESSAGE".into();
    failure
}
#[test] fn invoked_contract() {
    let holder = publish(problem());
    let debug = format!("{:?}",holder);
    assert!(debug.contains("Invalid"));
    assert!(!debug.contains("PRIVATE_FAILURE_MESSAGE"));
    let direct = publish_direct(problem());
    assert_eq!(direct.message,"PRIVATE_FAILURE_MESSAGE");
    let optional = publish_option(problem());
    assert!(optional.is_some());
    let result = publish_result(problem());
    assert!(result.is_err());
}
