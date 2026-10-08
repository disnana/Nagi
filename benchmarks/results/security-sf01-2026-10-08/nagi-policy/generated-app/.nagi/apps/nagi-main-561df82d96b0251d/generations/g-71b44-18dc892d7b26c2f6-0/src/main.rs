#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize, Clone, Copy)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_5374617465 {
    pub owner_subject: ::std::primitive::i64,
    pub blocked: ::std::primitive::bool,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_5374617465 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("State").field("owner_subject", &self.owner_subject).field("blocked", &self.blocked).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_5374617465 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["owner_subject","blocked"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
owner_subject: row.get(ix[0])?,
blocked: row.get(ix[1])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_446f63756d656e74 {
    pub id: ::std::primitive::i64,
    pub title: ::std::string::String,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_446f63756d656e74 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Document").field("id", &self.id).field("title", &self.title).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_446f63756d656e74 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["id","title"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
id: row.get(ix[0])?,
title: row.get(ix[1])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub enum __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_e_52656164 {
    Permission,
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_766572696679(mut request: ::nagi_runtime::http_server::Request, mut state: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_5374617465>) -> ::std::result::Result<::nagi_runtime::auth::VerifiedIdentity, ::nagi_runtime::auth::Failure> {
    native::verify(request, state).await
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_617574686f72697a655f72656164(mut scope: ::nagi_runtime::auth::AuthScope, mut request: ::nagi_runtime::http_server::Request, mut state: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_5374617465>) -> ::std::result::Result<::nagi_runtime::auth::Grant<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_e_52656164>, ::nagi_runtime::auth::Failure> {
    native::authorize_read(scope, request, state).await
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_726561645f646f63756d656e74(mut grant: ::nagi_runtime::auth::Grant<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_e_52656164>) -> ::std::result::Result<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_446f63756d656e74, ::nagi_runtime::auth::Failure> {
    native::read_document(grant).await
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_706f6c6963795f7061757365() -> () {
    native::policy_pause().await
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_7265706f7274() -> () {
    native::report()
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_696e697469616c697a65() -> () {
    native::initialize()
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_726561645f706f6c696379<'a>(mut scope: &'a ::nagi_runtime::auth::AuthScope, mut document_id: ::std::primitive::i64, mut owner_subject: ::std::primitive::i64, mut blocked: ::std::primitive::bool) -> ::std::result::Result<(), ::nagi_runtime::auth::Failure> {
    let mut scope: &::nagi_runtime::auth::AuthScope = scope;
    (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_706f6c6963795f7061757365()).await;
    if (((::nagi_runtime::auth::subject(scope) != owner_subject) || blocked) || (document_id != 1i64)) {
        return ::std::result::Result::Err(::nagi_runtime::auth::denied());
    }
    return ::std::result::Result::Ok(assert!(true));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_6865616c7468(mut request: ::nagi_runtime::http_server::Request, mut state: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_5374617465>, mut authority: ()) -> ::std::result::Result<::nagi_runtime::http_server::Response, ::nagi_runtime::Error> {
    return ::std::result::Result::Ok(::nagi_runtime::http_server::text(::nagi_runtime::http_server::Status::OK, "ok"));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_73686f775f646f63756d656e74(mut request: ::nagi_runtime::http_server::Request, mut state: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_5374617465>, mut grant: ::nagi_runtime::auth::Grant<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_e_52656164>) -> ::std::result::Result<::nagi_runtime::http_server::Response, ::nagi_runtime::Error> {
    match (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_726561645f646f63756d656e74(grant)).await {
        ::std::result::Result::Ok(mut document) => {
            return ::nagi_runtime::http_server::json::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_446f63756d656e74>(::nagi_runtime::http_server::Status::OK, &(document));
        },
        ::std::result::Result::Err(mut problem) => {
            return ::std::result::Result::Ok(::nagi_runtime::http_server::text(::nagi_runtime::http_server::Status::INTERNAL_SERVER_ERROR, ::nagi_runtime::auth::message(&(problem))));
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_main() -> ::std::result::Result<(), ::nagi_runtime::Error> {
    crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_696e697469616c697a65();
    let mut app: ::nagi_runtime::http_server::App<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_5374617465, ::nagi_runtime::Error> = ::nagi_runtime::http_server::app_default::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_5374617465>(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_5374617465 { owner_subject: 7i64, blocked: false });
    app = (::nagi_runtime::http_server::route(app, ::nagi_runtime::http_server::Method::GET, "/health", ::nagi_runtime::http_server::public_policy::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_5374617465>(), crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_6865616c7468))?;
    let mut policy: ::nagi_runtime::http_server::Policy<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_5374617465, ::nagi_runtime::auth::Grant<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_e_52656164>> = ::nagi_runtime::http_server::authorized_policy(crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_766572696679, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_617574686f72697a655f72656164);
    app = (::nagi_runtime::http_server::route(app, ::nagi_runtime::http_server::Method::GET, "/documents/{id}", policy, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_73686f775f646f63756d656e74))?;
    let mut port: ::std::primitive::i64 = (::nagi_runtime::parse_i64(&(match ::std::env::var("NAGI_SAMPLE_PORT") { ::std::result::Result::Ok(__nagi_env_value) => __nagi_env_value, ::std::result::Result::Err(_) => ("8098").to_owned() })))?;
    let mut outcome: ::std::result::Result<(), ::nagi_runtime::Error> = (::nagi_runtime::http_server::serve(app, port, ::nagi_runtime::http_server::default_options())).await;
    crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_7265706f7274();
    return outcome;
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_5374617465 as State;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_c_446f63756d656e74 as Document;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_e_52656164 as Read;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_766572696679 as verify;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_617574686f72697a655f72656164 as authorize_read;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_726561645f646f63756d656e74 as read_document;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_706f6c6963795f7061757365 as policy_pause;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_7265706f7274 as report;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_696e697469616c697a65 as initialize;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_726561645f706f6c696379 as read_policy;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_6865616c7468 as health;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630312f62656e63686d61726b732f726573756c74732f73656375726974792d736630312d323032362d31302d30382f6e6167692d706f6c6963792f70726f6a6563742f6d61696e2e6e616769_f_73686f775f646f63756d656e74 as show_document;
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
#[allow(non_snake_case)]
pub mod http {
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::Request as Request;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::Response as Response;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::Method as Method;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::Status as Status;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::Options as Options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::App as App;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::Policy as Policy;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::status as status;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::method as method;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::method_name as method_name;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::empty as empty;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::text as text;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::bytes as bytes;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::json as json;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::append_header as append_header;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::append_header_text as append_header_text;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::header as header;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::header_text as header_text;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::headers as headers;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::is_json_content_type as is_json_content_type;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::default_options as default_options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::options as options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::capacity as capacity;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::header_timeout as header_timeout;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::header_limits as header_limits;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::send_timeout as send_timeout;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::public_policy as public_policy;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::authenticated_policy as authenticated_policy;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::authorized_policy as authorized_policy;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::security_timeout as security_timeout;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::app as app;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::app_default as app_default;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::route as route;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::route_mapped as route_mapped;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::serve as serve;
}
fn main() { ::nagi_runtime::block_on(async {
if let ::std::result::Result::Err(e) = __nagi_main().await { eprintln!("{}",e); ::std::process::exit(1); }
}); }

#[path = "/workspace/Nagi-security-sf01/benchmarks/results/security-sf01-2026-10-08/nagi-policy/project/native.rs"]
mod native;
