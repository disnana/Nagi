#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
pub struct __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31332f68616e647772697474656e2e6c6f77_c_486f6c646572 {
    pub params: ::nagi_runtime::sqlite::Parameters,
}
#[allow(non_camel_case_types, non_snake_case)]
pub enum __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31332f68616e647772697474656e2e6c6f77_e_456e76656c6f7065 {
    Empty,
    Data {
        params: ::nagi_runtime::sqlite::Parameters,
    },
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31332f68616e647772697474656e2e6c6f77_f_686f6c64(mut params: ::nagi_runtime::sqlite::Parameters) -> __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31332f68616e647772697474656e2e6c6f77_c_486f6c646572 {
    return __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31332f68616e647772697474656e2e6c6f77_c_486f6c646572 { params: params };
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31332f68616e647772697474656e2e6c6f77_f_77726170(mut params: ::nagi_runtime::sqlite::Parameters) -> __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31332f68616e647772697474656e2e6c6f77_e_456e76656c6f7065 {
    return __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31332f68616e647772697474656e2e6c6f77_e_456e76656c6f7065::Data { params: params };
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31332f68616e647772697474656e2e6c6f77_c_486f6c646572 as Holder;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31332f68616e647772697474656e2e6c6f77_e_456e76656c6f7065 as Envelope;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31332f68616e647772697474656e2e6c6f77_f_686f6c64 as hold;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d31332f68616e647772697474656e2e6c6f77_f_77726170 as wrap;
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



#[test] fn invoked_contract() {
    let holder = hold(nagi_runtime::sqlite::bind_i64(nagi_runtime::sqlite::parameters(),3));
    let envelope = wrap(holder.params);
    match envelope { Envelope::Data { params } => { drop(params); }, _ => panic!("wrong variant") }
}
