#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize, Clone, Copy)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f6e6167692d676f6c64656e2f646174612d646572697665732f6d61696e2e6e616769_c_496e6c696e65 {
    pub value: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f6e6167692d676f6c64656e2f646174612d646572697665732f6d61696e2e6e616769_c_496e6c696e65 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Inline").field("value", &self.value).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f6e6167692d676f6c64656e2f646174612d646572697665732f6d61696e2e6e616769_c_496e6c696e65 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["value"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
value: row.get(ix[0])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f6e6167692d676f6c64656e2f646174612d646572697665732f6d61696e2e6e616769_c_4f776e6564 {
    pub label: ::std::string::String,
    pub values: ::std::vec::Vec<__nagi_def_2f6e6167692d676f6c64656e2f646174612d646572697665732f6d61696e2e6e616769_c_496e6c696e65>,
}
impl ::std::fmt::Debug for __nagi_def_2f6e6167692d676f6c64656e2f646174612d646572697665732f6d61696e2e6e616769_c_4f776e6564 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Owned").field("label", &self.label).field("values", &self.values).finish()
    }
}
#[allow(non_camel_case_types, non_snake_case)]
pub struct __nagi_def_2f6e6167692d676f6c64656e2f646174612d646572697665732f6d61696e2e6e616769_c_50726976617465 {
    pub cause: ::nagi_runtime::Error,
    pub database: ::nagi_runtime::sqlite::Pool,
}
impl ::std::fmt::Debug for __nagi_def_2f6e6167692d676f6c64656e2f646174612d646572697665732f6d61696e2e6e616769_c_50726976617465 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Private").field("cause", &self.cause).field("database", &self.database).finish()
    }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub enum __nagi_def_2f6e6167692d676f6c64656e2f646174612d646572697665732f6d61696e2e6e616769_e_436f707943686f696365 {
    Empty,
    Number {
        value: ::std::option::Option<::std::primitive::i64>,
    },
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(Debug)]
pub enum __nagi_def_2f6e6167692d676f6c64656e2f646174612d646572697665732f6d61696e2e6e616769_e_5072697661746543686f696365 {
    Text {
        value: ::std::string::String,
    },
    Caused {
        value: ::nagi_runtime::Error,
    },
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f6e6167692d676f6c64656e2f646174612d646572697665732f6d61696e2e6e616769_c_496e6c696e65 as Inline;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f6e6167692d676f6c64656e2f646174612d646572697665732f6d61696e2e6e616769_c_4f776e6564 as Owned;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f6e6167692d676f6c64656e2f646174612d646572697665732f6d61696e2e6e616769_c_50726976617465 as Private;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f6e6167692d676f6c64656e2f646174612d646572697665732f6d61696e2e6e616769_e_436f707943686f696365 as CopyChoice;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f6e6167692d676f6c64656e2f646174612d646572697665732f6d61696e2e6e616769_e_5072697661746543686f696365 as PrivateChoice;
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
