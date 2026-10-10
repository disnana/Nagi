#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize, Clone, Copy)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_506f6f6c {
    pub id: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_506f6f6c {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Pool").field("id", &self.id).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_506f6f6c {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["id"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
id: row.get(ix[0])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize, Clone, Copy)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_5478 {
    pub id: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_5478 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Tx").field("id", &self.id).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_5478 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["id"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
id: row.get(ix[0])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize, Clone, Copy)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_4f7074696f6e73 {
    pub id: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_4f7074696f6e73 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Options").field("id", &self.id).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_4f7074696f6e73 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["id"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
id: row.get(ix[0])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize, Clone, Copy)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_4661696c757265 {
    pub id: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_4661696c757265 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Failure").field("id", &self.id).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_4661696c757265 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["id"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
id: row.get(ix[0])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize, Clone, Copy)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_486f6c646572 {
    pub pool: __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_506f6f6c,
    pub tx: __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_5478,
    pub options: __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_4f7074696f6e73,
    pub failure: __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_4661696c757265,
}
impl ::std::fmt::Debug for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_486f6c646572 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Holder").field("pool", &self.pool).field("tx", &self.tx).field("options", &self.options).field("failure", &self.failure).finish()
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_f_7075626c697368(mut tx: __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_5478) -> ::std::sync::Arc<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_5478> {
    return ::std::sync::Arc::new(tx);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_f_6e61746976655f6964656e74697479(mut pool: ::nagi_runtime::sqlite::Pool) -> ::nagi_runtime::sqlite::Pool {
    return ::nagi_runtime::sqlite::clone_pool(&(pool));
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_506f6f6c as Pool;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_5478 as Tx;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_4f7074696f6e73 as Options;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_4661696c757265 as Failure;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_c_486f6c646572 as Holder;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_f_7075626c697368 as publish;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31312f68616e647772697474656e2e6c6f77_f_6e61746976655f6964656e74697479 as native_identity;
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
    let shared = publish(Tx { id: 7 });
    assert_eq!(shared.id,7);
    let holder = Holder { pool: Pool { id:1 }, tx: Tx { id:2 }, options: Options { id:3 }, failure: Failure { id:4 } };
    assert_eq!(holder.pool.id+holder.tx.id+holder.options.id+holder.failure.id,10);
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let pool = nagi_runtime::sqlite::open(":memory:",nagi_runtime::sqlite::options(1,2,1000,0).unwrap()).await.unwrap();
        let alias = native_identity(nagi_runtime::sqlite::clone_pool(&pool));
        nagi_runtime::sqlite::close(&alias,1000).await.unwrap();
    });
}
