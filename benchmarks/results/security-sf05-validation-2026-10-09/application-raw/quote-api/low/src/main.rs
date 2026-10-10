#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_436f6e666967 {
    pub currency: ::std::string::String,
    pub unit_price_minor: ::std::primitive::i64,
    pub shipping_minor: ::std::primitive::i64,
    pub free_shipping_quantity: ::std::primitive::i64,
    pub maximum_quantity: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_436f6e666967 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Config").field("currency", &self.currency).field("unit_price_minor", &self.unit_price_minor).field("shipping_minor", &self.shipping_minor).field("free_shipping_quantity", &self.free_shipping_quantity).field("maximum_quantity", &self.maximum_quantity).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_436f6e666967 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["currency","unit_price_minor","shipping_minor","free_shipping_quantity","maximum_quantity"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
currency: row.get(ix[0])?,
unit_price_minor: row.get(ix[1])?,
shipping_minor: row.get(ix[2])?,
free_shipping_quantity: row.get(ix[3])?,
maximum_quantity: row.get(ix[4])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_51756f7465496e707574 {
    pub sku: ::std::string::String,
    pub quantity: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_51756f7465496e707574 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("QuoteInput").field("sku", &self.sku).field("quantity", &self.quantity).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_51756f7465496e707574 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["sku","quantity"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
sku: row.get(ix[0])?,
quantity: row.get(ix[1])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_51756f7465 {
    pub sku: ::std::string::String,
    pub quantity: ::std::primitive::i64,
    pub currency: ::std::string::String,
    pub unit_price_minor: ::std::primitive::i64,
    pub subtotal_minor: ::std::primitive::i64,
    pub shipping_minor: ::std::primitive::i64,
    pub total_minor: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_51756f7465 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Quote").field("sku", &self.sku).field("quantity", &self.quantity).field("currency", &self.currency).field("unit_price_minor", &self.unit_price_minor).field("subtotal_minor", &self.subtotal_minor).field("shipping_minor", &self.shipping_minor).field("total_minor", &self.total_minor).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_51756f7465 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["sku","quantity","currency","unit_price_minor","subtotal_minor","shipping_minor","total_minor"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
sku: row.get(ix[0])?,
quantity: row.get(ix[1])?,
currency: row.get(ix[2])?,
unit_price_minor: row.get(ix[3])?,
subtotal_minor: row.get(ix[4])?,
shipping_minor: row.get(ix[5])?,
total_minor: row.get(ix[6])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4865616c7468 {
    pub status: ::std::string::String,
    pub currency: ::std::string::String,
    pub maximum_quantity: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4865616c7468 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Health").field("status", &self.status).field("currency", &self.currency).field("maximum_quantity", &self.maximum_quantity).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4865616c7468 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["status","currency","maximum_quantity"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
status: row.get(ix[0])?,
currency: row.get(ix[1])?,
maximum_quantity: row.get(ix[2])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4170694661696c757265 {
    pub problem: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72,
    pub request_id: ::std::string::String,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4170694661696c757265 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("ApiFailure").field("problem", &self.problem).field("request_id", &self.request_id).finish()
    }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4572726f72426f6479 {
    pub code: ::std::string::String,
    pub message: ::std::string::String,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4572726f72426f6479 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("ErrorBody").field("code", &self.code).field("message", &self.message).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4572726f72426f6479 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["code","message"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
code: row.get(ix[0])?,
message: row.get(ix[1])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub enum __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72 {
    InvalidRequestId,
    InvalidHeader,
    UnsupportedMediaType,
    InvalidJson,
    InvalidQuantity,
    UnknownSku,
    Internal,
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_776974685f726571756573745f6964<'a>(mut response: ::nagi_runtime::http_server::Response, mut request_id: &'a ::std::primitive::str) -> ::std::result::Result<::nagi_runtime::http_server::Response, ::nagi_runtime::Error> {
    let mut request_id: &::std::primitive::str = request_id;
    if (((request_id).len() as ::std::primitive::i64) == 0i64) {
        return ::std::result::Result::Ok(response);
    }
    return ::nagi_runtime::http_server::append_header_text(response, "X-Request-ID", request_id);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6572726f725f726573706f6e7365<'a>(mut status: ::nagi_runtime::http_server::Status, mut code: ::std::string::String, mut message: ::std::string::String, mut request_id: &'a ::std::primitive::str) -> ::nagi_runtime::http_server::Response {
    let mut request_id: &::std::primitive::str = request_id;
    let mut payload: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4572726f72426f6479 = __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4572726f72426f6479 { code: code, message: message };
    match ::nagi_runtime::http_server::json::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4572726f72426f6479>(status, &(payload)) {
        ::std::result::Result::Ok(mut response) => {
            match crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_776974685f726571756573745f6964(response, request_id) {
                ::std::result::Result::Ok(mut output) => {
                    return output;
                },
                ::std::result::Result::Err(_) => {
                    return ::nagi_runtime::http_server::empty(::nagi_runtime::http_server::Status::INTERNAL_SERVER_ERROR);
                },
            }
        },
        ::std::result::Result::Err(_) => {
            return ::nagi_runtime::http_server::empty(::nagi_runtime::http_server::Status::INTERNAL_SERVER_ERROR);
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6170695f6661696c757265(mut failure: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4170694661696c757265) -> ::nagi_runtime::http_server::Response {
    match (failure).problem {
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72::InvalidRequestId => {
            return crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6572726f725f726573706f6e7365(::nagi_runtime::http_server::Status::BAD_REQUEST, ::std::string::String::from("invalid_request_id"), ::std::string::String::from("X-Request-ID must contain one UUID"), ((failure).request_id).as_str());
        },
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72::InvalidHeader => {
            return crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6572726f725f726573706f6e7365(::nagi_runtime::http_server::Status::BAD_REQUEST, ::std::string::String::from("invalid_header"), ::std::string::String::from("Content-Type must contain one valid media type"), ((failure).request_id).as_str());
        },
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72::UnsupportedMediaType => {
            return crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6572726f725f726573706f6e7365(::nagi_runtime::http_server::Status::UNSUPPORTED_MEDIA_TYPE, ::std::string::String::from("unsupported_media_type"), ::std::string::String::from("Content-Type must be application/json"), ((failure).request_id).as_str());
        },
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72::InvalidJson => {
            return crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6572726f725f726573706f6e7365(::nagi_runtime::http_server::Status::BAD_REQUEST, ::std::string::String::from("invalid_json"), ::std::string::String::from("Expected exactly sku:string and quantity:integer"), ((failure).request_id).as_str());
        },
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72::InvalidQuantity => {
            return crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6572726f725f726573706f6e7365(::nagi_runtime::http_server::Status::UNPROCESSABLE_CONTENT, ::std::string::String::from("invalid_quantity"), ::std::string::String::from("Quantity is outside the configured range"), ((failure).request_id).as_str());
        },
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72::UnknownSku => {
            return crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6572726f725f726573706f6e7365(::nagi_runtime::http_server::Status::UNPROCESSABLE_CONTENT, ::std::string::String::from("unknown_sku"), ::std::string::String::from("Only NOTEBOOK is available"), ((failure).request_id).as_str());
        },
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72::Internal => {
            return crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6572726f725f726573706f6e7365(::nagi_runtime::http_server::Status::INTERNAL_SERVER_ERROR, ::std::string::String::from("internal_error"), ::std::string::String::from("Unable to create response"), ((failure).request_id).as_str());
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6865616c74685f6661696c757265(mut problem: ::nagi_runtime::Error) -> ::nagi_runtime::http_server::Response {
    return crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6572726f725f726573706f6e7365(::nagi_runtime::http_server::Status::INTERNAL_SERVER_ERROR, ::std::string::String::from("internal_error"), ::std::string::String::from("Unable to create response"), "");
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_726561645f726571756573745f6964<'a>(mut request: &'a ::nagi_runtime::http_server::Request) -> ::std::result::Result<::std::string::String, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4170694661696c757265> {
    let mut request: &::nagi_runtime::http_server::Request = request;
    match ::nagi_runtime::http_server::header_text(request, "X-Request-ID") {
        ::std::result::Result::Ok(mut header) => {
            match header {
                ::std::option::Option::Some(mut value) => {
                    match ::nagi_runtime::Uuid::parse(&(value)) {
                        ::std::result::Result::Ok(mut identifier) => {
                            return ::std::result::Result::Ok(identifier.to_string());
                        },
                        ::std::result::Result::Err(_) => {
                            return ::std::result::Result::Err(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4170694661696c757265 { problem: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72::InvalidRequestId, request_id: ::std::string::String::from("") });
                        },
                    }
                },
                ::std::option::Option::None => {
                    return ::std::result::Result::Ok(::std::string::String::from(""));
                },
            }
        },
        ::std::result::Result::Err(_) => {
            return ::std::result::Result::Err(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4170694661696c757265 { problem: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72::InvalidRequestId, request_id: ::std::string::String::from("") });
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_696e76616c69645f686561646572(mut cause: ::nagi_runtime::Error) -> __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72 {
    return __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72::InvalidHeader;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_696e76616c69645f6a736f6e(mut cause: ::nagi_runtime::Error) -> __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72 {
    return __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72::InvalidJson;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_726573706f6e73655f6572726f72(mut cause: ::nagi_runtime::Error) -> __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72 {
    return __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72::Internal;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_726571756972655f6a736f6e<'a>(mut request: &'a ::nagi_runtime::http_server::Request) -> ::std::result::Result<(), __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72> {
    let mut request: &::nagi_runtime::http_server::Request = request;
    let mut matches: ::std::primitive::bool = (::nagi_runtime::result::map_error(::nagi_runtime::http_server::is_json_content_type(request), crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_696e76616c69645f686561646572))?;
    if matches {
        return ::std::result::Result::Ok(assert!(true));
    }
    return ::std::result::Result::Err(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72::UnsupportedMediaType);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_726561645f696e707574<'a>(mut body: &'a [::std::primitive::u8]) -> ::std::result::Result<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_51756f7465496e707574, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72> {
    let mut body: &[::std::primitive::u8] = body;
    return ::nagi_runtime::result::map_error(::nagi_runtime::decode::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_51756f7465496e707574>(&(body)), crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_696e76616c69645f6a736f6e);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_63616c63756c617465(mut input: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_51756f7465496e707574, mut config: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_436f6e666967>) -> ::std::result::Result<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_51756f7465, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72> {
    if (((input).quantity < 1i64) || ((input).quantity > (config).maximum_quantity)) {
        return ::std::result::Result::Err(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72::InvalidQuantity);
    }
    if (((input).sku).as_str() != "NOTEBOOK") {
        return ::std::result::Result::Err(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72::UnknownSku);
    }
    let mut subtotal: ::std::primitive::i64 = ((input).quantity * (config).unit_price_minor);
    let mut shipping: ::std::primitive::i64 = (config).shipping_minor;
    if ((input).quantity >= (config).free_shipping_quantity) {
        shipping = 0i64;
    }
    return ::std::result::Result::Ok(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_51756f7465 { sku: (input).sku, quantity: (input).quantity, currency: (((config).currency).as_str()).to_owned(), unit_price_minor: (config).unit_price_minor, subtotal_minor: subtotal, shipping_minor: shipping, total_minor: (subtotal + shipping) });
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_71756f74655f726573706f6e7365(mut request: ::nagi_runtime::http_server::Request, mut config: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_436f6e666967>) -> ::std::result::Result<::nagi_runtime::http_server::Response, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72> {
    (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_726571756972655f6a736f6e(&(request)))?;
    let mut input: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_51756f7465496e707574 = (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_726561645f696e707574((request).body()))?;
    let mut value: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_51756f7465 = (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_63616c63756c617465(input, config))?;
    return ::nagi_runtime::result::map_error(::nagi_runtime::http_server::json::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_51756f7465>(::nagi_runtime::http_server::Status::OK, &(value)), crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_726573706f6e73655f6572726f72);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_71756f7465(mut request: ::nagi_runtime::http_server::Request, mut config: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_436f6e666967>, mut authority: ()) -> ::std::result::Result<::nagi_runtime::http_server::Response, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4170694661696c757265> {
    let mut request_id: ::std::string::String = (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_726561645f726571756573745f6964(&(request)))?;
    match crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_71756f74655f726573706f6e7365(request, config) {
        ::std::result::Result::Ok(mut response) => {
            match crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_776974685f726571756573745f6964(response, (request_id).as_str()) {
                ::std::result::Result::Ok(mut output) => {
                    return ::std::result::Result::Ok(output);
                },
                ::std::result::Result::Err(_) => {
                    return ::std::result::Result::Err(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4170694661696c757265 { problem: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72::Internal, request_id: request_id });
                },
            }
        },
        ::std::result::Result::Err(mut problem) => {
            return ::std::result::Result::Err(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4170694661696c757265 { problem: problem, request_id: request_id });
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6865616c7468(mut request: ::nagi_runtime::http_server::Request, mut config: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_436f6e666967>, mut authority: ()) -> ::std::result::Result<::nagi_runtime::http_server::Response, ::nagi_runtime::Error> {
    let mut value: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4865616c7468 = __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4865616c7468 { status: ::std::string::String::from("ok"), currency: (((config).currency).as_str()).to_owned(), maximum_quantity: (config).maximum_quantity };
    return ::nagi_runtime::http_server::json::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4865616c7468>(::nagi_runtime::http_server::Status::OK, &(value));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_main() -> ::std::result::Result<(), ::nagi_runtime::Error> {
    let mut maximum: ::std::primitive::i64 = (::nagi_runtime::parse_i64(&(match ::std::env::var("NAGI_SAMPLE_MAX_QUANTITY") { ::std::result::Result::Ok(__nagi_env_value) => __nagi_env_value, ::std::result::Result::Err(_) => ("1000").to_owned() })))?;
    if ((maximum < 1i64) || (maximum > 1000i64)) {
        return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("NAGI_SAMPLE_MAX_QUANTITY must be between 1 and 1000")));
    }
    let mut config: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_436f6e666967 = __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_436f6e666967 { currency: ::std::string::String::from("USD"), unit_price_minor: 1250i64, shipping_minor: 500i64, free_shipping_quantity: 5i64, maximum_quantity: maximum };
    let mut app: ::nagi_runtime::http_server::App<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_436f6e666967, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4170694661696c757265> = ::nagi_runtime::http_server::app::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_436f6e666967, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4170694661696c757265>(config, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6170695f6661696c757265);
    app = (::nagi_runtime::http_server::route(app, ::nagi_runtime::http_server::Method::POST, "/quotes", ::nagi_runtime::http_server::public_policy::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_436f6e666967>(), crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_71756f7465))?;
    app = (::nagi_runtime::http_server::route_mapped(app, ::nagi_runtime::http_server::Method::GET, "/health", ::nagi_runtime::http_server::public_policy::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_436f6e666967>(), crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6865616c7468, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6865616c74685f6661696c757265))?;
    let mut port: ::std::primitive::i64 = (::nagi_runtime::parse_i64(&(match ::std::env::var("NAGI_SAMPLE_PORT") { ::std::result::Result::Ok(__nagi_env_value) => __nagi_env_value, ::std::result::Result::Err(_) => ("8092").to_owned() })))?;
    let mut limits: ::nagi_runtime::http_server::Options = (::nagi_runtime::http_server::options(4096i64, 5000i64, 2000i64, 1000i64))?;
    return (::nagi_runtime::http_server::serve(app, port, limits)).await;
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_436f6e666967 as Config;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_51756f7465496e707574 as QuoteInput;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_51756f7465 as Quote;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4865616c7468 as Health;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4170694661696c757265 as ApiFailure;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_c_4572726f72426f6479 as ErrorBody;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_e_51756f74654572726f72 as QuoteError;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_776974685f726571756573745f6964 as with_request_id;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6572726f725f726573706f6e7365 as error_response;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6170695f6661696c757265 as api_failure;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6865616c74685f6661696c757265 as health_failure;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_726561645f726571756573745f6964 as read_request_id;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_696e76616c69645f686561646572 as invalid_header;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_696e76616c69645f6a736f6e as invalid_json;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_726573706f6e73655f6572726f72 as response_error;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_726571756972655f6a736f6e as require_json;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_726561645f696e707574 as read_input;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_63616c63756c617465 as calculate;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_71756f74655f726573706f6e7365 as quote_response;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_71756f7465 as quote;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f71756f74652d6170692f6d61696e2e6e616769_f_6865616c7468 as health;
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
#[allow(non_snake_case)]
pub mod result {
    #[allow(unused_imports)]
    pub use ::nagi_runtime::result::map_error as map_error;
}
fn main() { ::nagi_runtime::block_on(async {
if let ::std::result::Result::Err(e) = __nagi_main().await { eprintln!("{}",e); ::std::process::exit(1); }
}); }
