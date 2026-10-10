#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_c_5374617465 {
    pub expected: ::std::string::String,
}
impl ::std::fmt::Debug for __nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_c_5374617465 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("State").field("expected", &self.expected).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_c_5374617465 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["expected"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
expected: row.get(ix[0])?,
}) }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_f_616c6c6f776564<'a>(mut request: &'a ::nagi_runtime::http_server::Request, mut state: ::std::sync::Arc<__nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_c_5374617465>) -> ::std::result::Result<::std::primitive::bool, ::nagi_runtime::Error> {
    let mut request: &::nagi_runtime::http_server::Request = request;
    assert!(((request).method == ::nagi_runtime::http_server::Method::GET));
    assert!(((request).path() == "/probe"));
    assert!(((request).path() < "/z"));
    assert!(("/a" < (request).path()));
    assert!(((request).path() <= ((state).expected).as_str()));
    assert!((((state).expected).as_str() >= (request).path()));
    assert!(((request).body() <= (request).body()));
    assert!(((((request).body()).len() as ::std::primitive::i64) == 0i64));
    let mut status: ::nagi_runtime::http_server::Status = *(&(::nagi_runtime::http_server::Status::OK));
    assert!(((status).value() == 200i64));
    match (::nagi_runtime::http_server::header_text(request, "Authorization"))? {
        ::std::option::Option::Some(mut value) => {
            return ::std::result::Result::Ok((value == ((state).expected).as_str()));
        },
        ::std::option::Option::None => {
            return ::std::result::Result::Ok(false);
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_f_68616e646c6572(mut request: ::nagi_runtime::http_server::Request, mut state: ::std::sync::Arc<__nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_c_5374617465>, mut access: ()) -> ::std::result::Result<::nagi_runtime::http_server::Response, ::nagi_runtime::Error> {
    let mut accepted: ::std::primitive::bool = (crate::__nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_f_616c6c6f776564(&(request), state))?;
    if accepted {
        return ::std::result::Result::Ok(::nagi_runtime::http_server::empty(::nagi_runtime::http_server::Status::OK));
    } else {
        return ::std::result::Result::Ok(::nagi_runtime::http_server::empty(::nagi_runtime::http_server::Status::UNAUTHORIZED));
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_f_7265676973746572(mut state: __nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_c_5374617465) -> ::std::result::Result<::nagi_runtime::http_server::App<__nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_c_5374617465, ::nagi_runtime::Error>, ::nagi_runtime::Error> {
    let mut app: ::nagi_runtime::http_server::App<__nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_c_5374617465, ::nagi_runtime::Error> = ::nagi_runtime::http_server::app_default::<__nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_c_5374617465>(state);
    app = (::nagi_runtime::http_server::route(app, ::nagi_runtime::http_server::Method::GET, "/probe", ::nagi_runtime::http_server::public_policy::<__nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_c_5374617465>(), crate::__nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_f_68616e646c6572))?;
    return ::std::result::Result::Ok(app);
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_c_5374617465 as State;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_f_616c6c6f776564 as allowed;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_f_68616e646c6572 as handler;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f6e6167692d676f6c64656e2f687474702d696e7370656374696f6e2f6d61696e2e6e616769_f_7265676973746572 as register;
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
    pub use ::nagi_runtime::http_server::authority as authority;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::trusted_proxy as trusted_proxy;
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
fn main() {}
