#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d {
    pub id: ::std::primitive::i64,
    pub name: ::std::string::String,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Item").field("id", &self.id).field("name", &self.name).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["id","name"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
id: row.get(ix[0])?,
name: row.get(ix[1])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize, Clone, Copy)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4e756d626572 {
    pub value: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4e756d626572 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Number").field("value", &self.value).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4e756d626572 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["value"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
value: row.get(ix[0])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465 {
    pub db: ::nagi_runtime::sqlite::Pool,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("State").field("db", &self.db).finish()
    }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_4572726f72426f6479 {
    pub error: ::std::string::String,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_4572726f72426f6479 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("ErrorBody").field("error", &self.error).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_4572726f72426f6479 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["error"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
error: row.get(ix[0])?,
}) }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_726561645f6f7074696f6e616c(mut state: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465>) -> ::std::result::Result<::std::option::Option<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d>, ::nagi_runtime::Error> {
    return (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73746f726167655f726561645f6f7074696f6e616c(&((state).db), 1i64)).await;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_726561645f6974656d(mut state: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465>, mut id: ::std::primitive::i64) -> ::std::result::Result<::std::option::Option<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d>, ::nagi_runtime::Error> {
    return (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73746f726167655f726561645f6974656d(&((state).db), id)).await;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_726573706f6e73655f6572726f72(mut status: ::nagi_runtime::http_server::Status, mut message: ::std::string::String) -> ::nagi_runtime::http_server::Response {
    match ::nagi_runtime::http_server::json::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_4572726f72426f6479>(status, &(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_4572726f72426f6479 { error: message })) {
        ::std::result::Result::Ok(mut response) => {
            return response;
        },
        ::std::result::Result::Err(_) => {
            return ::nagi_runtime::http_server::empty(::nagi_runtime::http_server::Status::INTERNAL_SERVER_ERROR);
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_6170695f6572726f72(mut problem: ::nagi_runtime::Error) -> ::nagi_runtime::http_server::Response {
    let mut kind: ::std::string::String = ::nagi_runtime::error_kind(&(problem)).to_owned();
    if ((kind).as_str() == "invalid") {
        return crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_726573706f6e73655f6572726f72(::nagi_runtime::http_server::Status::BAD_REQUEST, (problem).message.clone());
    }
    if ((kind).as_str() == "not_found") {
        return crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_726573706f6e73655f6572726f72(::nagi_runtime::http_server::Status::NOT_FOUND, (problem).message.clone());
    }
    return crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_726573706f6e73655f6572726f72(::nagi_runtime::http_server::Status::INTERNAL_SERVER_ERROR, ::std::string::String::from("internal error"));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_71756572795f76616c7565<'a>(mut request: &'a ::nagi_runtime::http_server::Request) -> ::std::result::Result<::std::string::String, ::nagi_runtime::Error> {
    let mut request: &::nagi_runtime::http_server::Request = request;
    match (request).query() {
        ::std::option::Option::Some(mut query) => {
            if ((((query).len() as ::std::primitive::i64) < 7i64) || ((::nagi_runtime::slice_str(query, 0i64, 6i64))? != "value=")) {
                return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("value must be an integer")));
            }
            return ::std::result::Result::Ok(((::nagi_runtime::slice_str(query, 6i64, ((query).len() as ::std::primitive::i64)))?).to_owned());
        },
        ::std::option::Option::None => {
            return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("value must be an integer")));
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_6974656d5f6964<'a>(mut request: &'a ::nagi_runtime::http_server::Request) -> ::std::result::Result<::std::primitive::i64, ::nagi_runtime::Error> {
    let mut request: &::nagi_runtime::http_server::Request = request;
    if ((((request).path()).len() as ::std::primitive::i64) <= 11i64) {
        return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("id must be positive")));
    }
    return ::nagi_runtime::parse_i64(&((::nagi_runtime::slice_str((request).path(), 11i64, (((request).path()).len() as ::std::primitive::i64)))?));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_6865616c7468(mut request: ::nagi_runtime::http_server::Request, mut state: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465>, mut authority: ()) -> ::std::result::Result<::nagi_runtime::http_server::Response, ::nagi_runtime::Error> {
    return ::std::result::Result::Ok(::nagi_runtime::http_server::text(::nagi_runtime::http_server::Status::OK, "ok"));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_646f75626c65(mut request: ::nagi_runtime::http_server::Request, mut state: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465>, mut authority: ()) -> ::std::result::Result<::nagi_runtime::http_server::Response, ::nagi_runtime::Error> {
    let mut value: ::std::string::String = (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_71756572795f76616c7565(&(request)))?;
    match ::nagi_runtime::parse_i64(&((value).as_str())) {
        ::std::result::Result::Ok(mut number) => {
            if ((number < 0i64) || (number > 1000000i64)) {
                return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("value must be between 0 and 1000000")));
            }
            return ::nagi_runtime::http_server::json::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4e756d626572>(::nagi_runtime::http_server::Status::OK, &(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4e756d626572 { value: (number * 2i64) }));
        },
        ::std::result::Result::Err(_) => {
            return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("value must be an integer")));
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_6765745f6974656d(mut request: ::nagi_runtime::http_server::Request, mut state: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465>, mut authority: ()) -> ::std::result::Result<::nagi_runtime::http_server::Response, ::nagi_runtime::Error> {
    let mut id: ::std::primitive::i64 = (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_6974656d5f6964(&(request)))?;
    if (id < 1i64) {
        return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("id must be positive")));
    }
    if (id > 1000000i64) {
        return ::std::result::Result::Err(::nagi_runtime::Error { kind: ::nagi_runtime::ErrorKind::NotFound, message: ::std::string::String::from("item is outside the sample range") });
    }
    match (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_726561645f6974656d(state, id)).await {
        ::std::result::Result::Ok(mut item) => {
            match item {
                ::std::option::Option::Some(mut value) => {
                    return ::nagi_runtime::http_server::json::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d>(::nagi_runtime::http_server::Status::OK, &(value));
                },
                ::std::option::Option::None => {
                    return ::std::result::Result::Err(::nagi_runtime::Error { kind: ::nagi_runtime::ErrorKind::NotFound, message: ::std::string::String::from("not found") });
                },
            }
        },
        ::std::result::Result::Err(mut problem) => {
            return ::std::result::Result::Err(problem);
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_66616c6c6261636b(mut request: ::nagi_runtime::http_server::Request, mut state: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465>, mut authority: ()) -> ::std::result::Result<::nagi_runtime::http_server::Response, ::nagi_runtime::Error> {
    match (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_726561645f6f7074696f6e616c(state)).await {
        ::std::result::Result::Ok(mut item) => {
            match item {
                ::std::option::Option::Some(mut value) => {
                    return ::nagi_runtime::http_server::json::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d>(::nagi_runtime::http_server::Status::OK, &(value));
                },
                ::std::option::Option::None => {
                    return ::std::result::Result::Err(::nagi_runtime::Error { kind: ::nagi_runtime::ErrorKind::NotFound, message: ::std::string::String::from("not found") });
                },
            }
        },
        ::std::result::Result::Err(mut problem) => {
            println!("{}", ::nagi_runtime::error_kind(&(problem)));
            return ::nagi_runtime::http_server::json::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d>(::nagi_runtime::http_server::Status::OK, &(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d { id: 0i64, name: ::std::string::String::from("cached item") }));
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_64617461626173655f6572726f72(mut request: ::nagi_runtime::http_server::Request, mut state: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465>, mut authority: ()) -> ::std::result::Result<::nagi_runtime::http_server::Response, ::nagi_runtime::Error> {
    match (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_726561645f6f7074696f6e616c(state)).await {
        ::std::result::Result::Ok(mut item) => {
            match item {
                ::std::option::Option::Some(mut value) => {
                    return ::nagi_runtime::http_server::json::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d>(::nagi_runtime::http_server::Status::OK, &(value));
                },
                ::std::option::Option::None => {
                    return ::std::result::Result::Err(::nagi_runtime::Error { kind: ::nagi_runtime::ErrorKind::NotFound, message: ::std::string::String::from("not found") });
                },
            }
        },
        ::std::result::Result::Err(mut problem) => {
            println!("{}", ::nagi_runtime::error_kind(&(problem)));
            return ::std::result::Result::Err(problem);
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_696e7465726e616c5f6661696c757265(mut request: ::nagi_runtime::http_server::Request, mut state: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465>, mut authority: ()) -> ::std::result::Result<::nagi_runtime::http_server::Response, ::nagi_runtime::Error> {
    return ::std::result::Result::Err(::nagi_runtime::Error::internal(::std::string::String::from("sample internal failure")));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_main() -> ::std::result::Result<(), ::nagi_runtime::Error> {
    let mut port: ::std::primitive::i64 = (::nagi_runtime::parse_i64(&(match ::std::env::var("NAGI_PORT") { ::std::result::Result::Ok(__nagi_env_value) => __nagi_env_value, ::std::result::Result::Err(_) => ("8097").to_owned() })))?;
    if ((port < 1i64) || (port > 65535i64)) {
        return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("NAGI_PORT must be between 1 and 65535")));
    }
    let mut db: ::nagi_runtime::sqlite::Pool = ((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_6f70656e5f6461746162617365(":memory:")).await)?;
    ((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_626f6f7473747261705f6974656d73(&(db))).await)?;
    ((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_736565645f6974656d73(&(db))).await)?;
    let mut app: ::nagi_runtime::http_server::App<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465, ::nagi_runtime::Error> = ::nagi_runtime::http_server::app::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465, ::nagi_runtime::Error>(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465 { db: db }, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_6170695f6572726f72);
    app = (::nagi_runtime::http_server::route(app, ::nagi_runtime::http_server::Method::GET, "/health", ::nagi_runtime::http_server::public_policy::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465>(), crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_6865616c7468))?;
    app = (::nagi_runtime::http_server::route(app, ::nagi_runtime::http_server::Method::GET, "/api/double", ::nagi_runtime::http_server::public_policy::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465>(), crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_646f75626c65))?;
    app = (::nagi_runtime::http_server::route(app, ::nagi_runtime::http_server::Method::GET, "/api/items/{id}", ::nagi_runtime::http_server::public_policy::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465>(), crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_6765745f6974656d))?;
    app = (::nagi_runtime::http_server::route(app, ::nagi_runtime::http_server::Method::GET, "/api/fallback", ::nagi_runtime::http_server::public_policy::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465>(), crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_66616c6c6261636b))?;
    app = (::nagi_runtime::http_server::route(app, ::nagi_runtime::http_server::Method::GET, "/api/db-error", ::nagi_runtime::http_server::public_policy::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465>(), crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_64617461626173655f6572726f72))?;
    app = (::nagi_runtime::http_server::route(app, ::nagi_runtime::http_server::Method::GET, "/api/internal-error", ::nagi_runtime::http_server::public_policy::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465>(), crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_696e7465726e616c5f6661696c757265))?;
    return (::nagi_runtime::http_server::serve(app, port, ::nagi_runtime::http_server::default_options())).await;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73716c6974655f6572726f72(mut problem: ::nagi_runtime::sqlite::Failure) -> ::nagi_runtime::Error {
    match ::nagi_runtime::sqlite::copy_primary_error(&(problem)) {
        ::std::option::Option::Some(mut cause) => {
            return cause;
        },
        ::std::option::Option::None => {
            let mut fallback: ::std::result::Result<::nagi_runtime::Error, ::nagi_runtime::Error> = ::std::result::Result::Err(::nagi_runtime::Error::internal(::std::string::String::from("SQLite operation failed")));
            match fallback {
                ::std::result::Result::Ok(mut cause) => {
                    return cause;
                },
                ::std::result::Result::Err(mut cause) => {
                    return cause;
                },
            }
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_6f70656e5f6461746162617365<'a>(mut path: &'a ::std::primitive::str) -> ::std::result::Result<::nagi_runtime::sqlite::Pool, ::nagi_runtime::Error> {
    let mut path: &::std::primitive::str = path;
    let mut config: ::nagi_runtime::sqlite::Options = (::nagi_runtime::sqlite::options(1i64, 2i64, 1000i64, 0i64))?;
    return ::nagi_runtime::result::map_error((::nagi_runtime::sqlite::open(path, config)).await, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73716c6974655f6572726f72);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73746f726167655f726561645f6f7074696f6e616c<'a>(mut pool: &'a ::nagi_runtime::sqlite::Pool, mut value0: ::std::primitive::i64) -> ::std::result::Result<::std::option::Option<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d>, ::nagi_runtime::Error> {
    let mut pool: &::nagi_runtime::sqlite::Pool = pool;
    let mut tx: ::nagi_runtime::sqlite::Tx = (::nagi_runtime::result::map_error((::nagi_runtime::sqlite::begin(pool, ::nagi_runtime::sqlite::BeginMode::Deferred)).await, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73716c6974655f6572726f72))?;
    let mut work: ::std::result::Result<::std::option::Option<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d>, ::nagi_runtime::sqlite::Failure> = (::nagi_runtime::sqlite::query::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d>(&(tx), ::nagi_runtime::sqlite::literal("SELECT id, name FROM optional_items WHERE id = ?"), ::nagi_runtime::sqlite::bind_i64(::nagi_runtime::sqlite::parameters(), value0))).await;
    let mut ending: ::std::result::Result<(), ::nagi_runtime::sqlite::Failure> = (::nagi_runtime::sqlite::rollback(tx)).await;
    let mut value: ::std::option::Option<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d> = (::nagi_runtime::result::map_error(work, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73716c6974655f6572726f72))?;
    (::nagi_runtime::result::map_error(ending, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73716c6974655f6572726f72))?;
    return ::std::result::Result::Ok(value);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73746f726167655f726561645f6974656d<'a>(mut pool: &'a ::nagi_runtime::sqlite::Pool, mut value0: ::std::primitive::i64) -> ::std::result::Result<::std::option::Option<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d>, ::nagi_runtime::Error> {
    let mut pool: &::nagi_runtime::sqlite::Pool = pool;
    let mut tx: ::nagi_runtime::sqlite::Tx = (::nagi_runtime::result::map_error((::nagi_runtime::sqlite::begin(pool, ::nagi_runtime::sqlite::BeginMode::Deferred)).await, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73716c6974655f6572726f72))?;
    let mut work: ::std::result::Result<::std::option::Option<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d>, ::nagi_runtime::sqlite::Failure> = (::nagi_runtime::sqlite::query::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d>(&(tx), ::nagi_runtime::sqlite::literal("SELECT id, name FROM sample_items WHERE id = ?"), ::nagi_runtime::sqlite::bind_i64(::nagi_runtime::sqlite::parameters(), value0))).await;
    let mut ending: ::std::result::Result<(), ::nagi_runtime::sqlite::Failure> = (::nagi_runtime::sqlite::rollback(tx)).await;
    let mut value: ::std::option::Option<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d> = (::nagi_runtime::result::map_error(work, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73716c6974655f6572726f72))?;
    (::nagi_runtime::result::map_error(ending, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73716c6974655f6572726f72))?;
    return ::std::result::Result::Ok(value);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_626f6f7473747261705f6974656d73<'a>(mut pool: &'a ::nagi_runtime::sqlite::Pool) -> ::std::result::Result<::std::primitive::i64, ::nagi_runtime::Error> {
    let mut pool: &::nagi_runtime::sqlite::Pool = pool;
    let mut tx: ::nagi_runtime::sqlite::Tx = (::nagi_runtime::result::map_error((::nagi_runtime::sqlite::begin(pool, ::nagi_runtime::sqlite::BeginMode::Immediate)).await, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73716c6974655f6572726f72))?;
    let mut work: ::std::result::Result<::std::primitive::i64, ::nagi_runtime::sqlite::Failure> = (::nagi_runtime::sqlite::exec(&(tx), ::nagi_runtime::sqlite::literal("CREATE TABLE sample_items(id INTEGER PRIMARY KEY, name TEXT NOT NULL)"), ::nagi_runtime::sqlite::parameters())).await;
    let mut ending: ::std::result::Result<(), ::nagi_runtime::sqlite::Failure> = (::nagi_runtime::sqlite::commit(tx)).await;
    let mut value: ::std::primitive::i64 = (::nagi_runtime::result::map_error(work, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73716c6974655f6572726f72))?;
    (::nagi_runtime::result::map_error(ending, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73716c6974655f6572726f72))?;
    return ::std::result::Result::Ok(value);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_736565645f6974656d73<'a>(mut pool: &'a ::nagi_runtime::sqlite::Pool) -> ::std::result::Result<::std::primitive::i64, ::nagi_runtime::Error> {
    let mut pool: &::nagi_runtime::sqlite::Pool = pool;
    let mut tx: ::nagi_runtime::sqlite::Tx = (::nagi_runtime::result::map_error((::nagi_runtime::sqlite::begin(pool, ::nagi_runtime::sqlite::BeginMode::Immediate)).await, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73716c6974655f6572726f72))?;
    let mut work: ::std::result::Result<::std::primitive::i64, ::nagi_runtime::sqlite::Failure> = (::nagi_runtime::sqlite::exec(&(tx), ::nagi_runtime::sqlite::literal("INSERT INTO sample_items(id, name) VALUES (1, 'notebook')"), ::nagi_runtime::sqlite::parameters())).await;
    let mut ending: ::std::result::Result<(), ::nagi_runtime::sqlite::Failure> = (::nagi_runtime::sqlite::commit(tx)).await;
    let mut value: ::std::primitive::i64 = (::nagi_runtime::result::map_error(work, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73716c6974655f6572726f72))?;
    (::nagi_runtime::result::map_error(ending, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73716c6974655f6572726f72))?;
    return ::std::result::Result::Ok(value);
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_5374617465 as State;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_c_4572726f72426f6479 as ErrorBody;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_726561645f6f7074696f6e616c as read_optional;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_726561645f6974656d as read_item;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_726573706f6e73655f6572726f72 as response_error;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_6170695f6572726f72 as api_error;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_71756572795f76616c7565 as query_value;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_6974656d5f6964 as item_id;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_6865616c7468 as health;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_646f75626c65 as double;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_6765745f6974656d as get_item;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_66616c6c6261636b as fallback;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_64617461626173655f6572726f72 as database_error;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_696e7465726e616c5f6661696c757265 as internal_failure;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73716c6974655f6572726f72 as sqlite_error;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_6f70656e5f6461746162617365 as open_database;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73746f726167655f726561645f6f7074696f6e616c as storage_read_optional;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_73746f726167655f726561645f6974656d as storage_read_item;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_626f6f7473747261705f6974656d73 as bootstrap_items;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f7365727665722e6e616769_f_736565645f6974656d73 as seed_items;
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
#[allow(non_snake_case)]
pub mod result {
    #[allow(unused_imports)]
    pub use ::nagi_runtime::result::map_error as map_error;
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4974656d as Item;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f6275696c642f736630352d687474702d6578616d706c65732d66696e616c2d636c6f737572652f726573756c742d6170692d6d656139677534382f6d6f64656c732e6e616769_c_4e756d626572 as Number;
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
