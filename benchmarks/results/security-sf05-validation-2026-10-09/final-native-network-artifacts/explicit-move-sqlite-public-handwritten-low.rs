#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize, Clone, Copy)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d342f68616e647772697474656e2e6c6f77_c_546f74616c {
    pub total: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d342f68616e647772697474656e2e6c6f77_c_546f74616c {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Total").field("total", &self.total).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d342f68616e647772697474656e2e6c6f77_c_546f74616c {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["total"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
total: row.get(ix[0])?,
}) }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d342f68616e647772697474656e2e6c6f77_f_636f6e66696775726174696f6e() -> ::std::result::Result<::nagi_runtime::sqlite::Options, ::nagi_runtime::Error> {
    return ::nagi_runtime::sqlite::options(1i64, 2i64, 1000i64, 0i64);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d342f68616e647772697474656e2e6c6f77_f_6578657263697365(mut config: ::nagi_runtime::sqlite::Options) -> ::std::result::Result<::std::primitive::i64, ::nagi_runtime::sqlite::Failure> {
    let mut pool: ::nagi_runtime::sqlite::Pool = ((::nagi_runtime::sqlite::open(":memory:", config)).await)?;
    let mut alias: ::nagi_runtime::sqlite::Pool = ::nagi_runtime::sqlite::clone_pool(&(pool));
    let mut tx: ::nagi_runtime::sqlite::Tx = ((::nagi_runtime::sqlite::begin(&(pool), ::nagi_runtime::sqlite::BeginMode::Deferred)).await)?;
    ((::nagi_runtime::sqlite::exec(&(tx), ::nagi_runtime::sqlite::literal("CREATE TABLE amounts(amount INTEGER NOT NULL)"), ::nagi_runtime::sqlite::parameters())).await)?;
    let mut params: ::nagi_runtime::sqlite::Parameters = ::nagi_runtime::sqlite::bind_i64(::nagi_runtime::sqlite::parameters(), 7i64);
    ((::nagi_runtime::sqlite::exec(&(tx), ::nagi_runtime::sqlite::literal("INSERT INTO amounts VALUES (?)"), params)).await)?;
    ((::nagi_runtime::sqlite::commit(tx)).await)?;
    tx = ((::nagi_runtime::sqlite::begin(&(alias), ::nagi_runtime::sqlite::BeginMode::Deferred)).await)?;
    let mut sql: ::nagi_runtime::sqlite::Query = ::nagi_runtime::sqlite::literal("SELECT SUM(amount) AS total FROM amounts");
    let mut row: ::std::option::Option<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d342f68616e647772697474656e2e6c6f77_c_546f74616c> = ((::nagi_runtime::sqlite::query::<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d342f68616e647772697474656e2e6c6f77_c_546f74616c>(&(tx), sql, ::nagi_runtime::sqlite::parameters())).await)?;
    ((::nagi_runtime::sqlite::rollback(tx)).await)?;
    ((::nagi_runtime::sqlite::close(&(alias), 1000i64)).await)?;
    match row {
        ::std::option::Option::Some(mut value) => {
            return ::std::result::Result::Ok((value).total);
        },
        ::std::option::Option::None => {
            return ::std::result::Result::Ok(0i64);
        },
    }
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d342f68616e647772697474656e2e6c6f77_c_546f74616c as Total;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d342f68616e647772697474656e2e6c6f77_f_636f6e66696775726174696f6e as configuration;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d342f68616e647772697474656e2e6c6f77_f_6578657263697365 as exercise;
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



#[test]
fn real_public_sqlite() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let options = configuration().unwrap();
    assert_eq!(rt.block_on(exercise(options)).unwrap(), 7);
}
