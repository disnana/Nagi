#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f62656e63686d61726b732f73716c6974652d7075626c69632f6f7065726174696f6e732e6e616769_f_6c69746572616c<'a>(mut tx: &'a ::nagi_runtime::sqlite::Tx, mut value: ::std::primitive::i64) -> ::std::result::Result<::std::primitive::i64, ::nagi_runtime::sqlite::Failure> {
    let mut tx: &::nagi_runtime::sqlite::Tx = tx;
    let mut params: ::nagi_runtime::sqlite::Parameters = ::nagi_runtime::sqlite::bind_i64(::nagi_runtime::sqlite::parameters(), value);
    return (::nagi_runtime::sqlite::exec(tx, ::nagi_runtime::sqlite::literal("INSERT INTO data VALUES (?)"), params)).await;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f62656e63686d61726b732f73716c6974652d7075626c69632f6f7065726174696f6e732e6e616769_f_73656c6563746564<'a>(mut tx: &'a ::nagi_runtime::sqlite::Tx, mut sql: ::nagi_runtime::sqlite::Query, mut value: ::std::primitive::i64) -> ::std::result::Result<::std::primitive::i64, ::nagi_runtime::sqlite::Failure> {
    let mut tx: &::nagi_runtime::sqlite::Tx = tx;
    let mut params: ::nagi_runtime::sqlite::Parameters = ::nagi_runtime::sqlite::bind_i64(::nagi_runtime::sqlite::parameters(), value);
    return (::nagi_runtime::sqlite::exec(tx, sql, params)).await;
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f62656e63686d61726b732f73716c6974652d7075626c69632f6f7065726174696f6e732e6e616769_f_6c69746572616c as literal;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f62656e63686d61726b732f73716c6974652d7075626c69632f6f7065726174696f6e732e6e616769_f_73656c6563746564 as selected;
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
