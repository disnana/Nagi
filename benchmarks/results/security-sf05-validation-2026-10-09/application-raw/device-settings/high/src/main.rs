#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_44657669636553657474696e6773 {
    pub id: ::std::primitive::i64,
    pub enabled: ::std::option::Option<::std::primitive::bool>,
    pub gain: ::std::option::Option<::std::primitive::f64>,
    pub label: ::std::option::Option<::std::string::String>,
    pub calibration: ::std::option::Option<::std::vec::Vec<::std::primitive::u8>>,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_44657669636553657474696e6773 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("DeviceSettings").field("id", &self.id).field("enabled", &self.enabled).field("gain", &self.gain).field("label", &self.label).field("calibration", &self.calibration).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_44657669636553657474696e6773 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["id","enabled","gain","label","calibration"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
id: row.get(ix[0])?,
enabled: row.get(ix[1])?,
gain: row.get(ix[2])?,
label: row.get(ix[3])?,
calibration: row.get(ix[4])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_5374617465 {
    pub db: ::nagi_runtime::sqlite::Pool,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_5374617465 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("State").field("db", &self.db).finish()
    }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_4572726f72426f6479 {
    pub error: ::std::string::String,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_4572726f72426f6479 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("ErrorBody").field("error", &self.error).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_4572726f72426f6479 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["error"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
error: row.get(ix[0])?,
}) }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_6465766963655f6964<'a>(mut request: &'a ::nagi_runtime::http_server::Request) -> ::std::result::Result<::std::primitive::i64, ::nagi_runtime::Error> {
    let mut request: &::nagi_runtime::http_server::Request = request;
    if ((((request).path()).len() as ::std::primitive::i64) <= 9i64) {
        return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("device id is missing")));
    }
    return ::nagi_runtime::parse_i64(&((::nagi_runtime::slice_str((request).path(), 9i64, (((request).path()).len() as ::std::primitive::i64)))?));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_6d697373696e675f646576696365() -> ::std::result::Result<::nagi_runtime::http_server::Response, ::nagi_runtime::Error> {
    return ::nagi_runtime::http_server::json::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_4572726f72426f6479>(::nagi_runtime::http_server::Status::NOT_FOUND, &(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_4572726f72426f6479 { error: ::std::string::String::from("not found") }));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_6865616c7468(mut request: ::nagi_runtime::http_server::Request, mut state: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_5374617465>, mut authority: ()) -> ::std::result::Result<::nagi_runtime::http_server::Response, ::nagi_runtime::Error> {
    return ::std::result::Result::Ok(::nagi_runtime::http_server::text(::nagi_runtime::http_server::Status::OK, "ok"));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_66696e645f646576696365(mut request: ::nagi_runtime::http_server::Request, mut state: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_5374617465>, mut authority: ()) -> ::std::result::Result<::nagi_runtime::http_server::Response, ::nagi_runtime::Error> {
    let mut id: ::std::primitive::i64 = (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_6465766963655f6964(&(request)))?;
    match (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73746f726167655f66696e645f646576696365(&((state).db), id)).await {
        ::std::result::Result::Ok(mut found) => {
            match found {
                ::std::option::Option::Some(mut device) => {
                    return ::nagi_runtime::http_server::json::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_44657669636553657474696e6773>(::nagi_runtime::http_server::Status::OK, &(device));
                },
                ::std::option::Option::None => {
                    return crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_6d697373696e675f646576696365();
                },
            }
        },
        ::std::result::Result::Err(mut problem) => {
            return ::std::result::Result::Err(problem);
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_64657669636573(mut request: ::nagi_runtime::http_server::Request, mut state: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_5374617465>, mut authority: ()) -> ::std::result::Result<::nagi_runtime::http_server::Response, ::nagi_runtime::Error> {
    let mut rows: ::std::vec::Vec<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_44657669636553657474696e6773> = ((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73746f726167655f6c6973745f64657669636573(&((state).db))).await)?;
    return ::nagi_runtime::http_server::json::<::std::vec::Vec<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_44657669636553657474696e6773>>(::nagi_runtime::http_server::Status::OK, &(rows));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_main() -> ::std::result::Result<(), ::nagi_runtime::Error> {
    let mut path: ::std::string::String = match ::std::env::var("NAGI_SAMPLE_DB") { ::std::result::Result::Ok(__nagi_env_value) => __nagi_env_value, ::std::result::Result::Err(_) => ("settings.sqlite").to_owned() };
    let mut db: ::nagi_runtime::sqlite::Pool = ((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_6f70656e5f6461746162617365((path).as_str())).await)?;
    ((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_626f6f7473747261705f64657669636573(&(db))).await)?;
    ((crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_736565645f64657669636573(&(db))).await)?;
    let mut app: ::nagi_runtime::http_server::App<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_5374617465, ::nagi_runtime::Error> = ::nagi_runtime::http_server::app_default::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_5374617465>(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_5374617465 { db: db });
    app = (::nagi_runtime::http_server::route(app, ::nagi_runtime::http_server::Method::GET, "/health", ::nagi_runtime::http_server::public_policy::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_5374617465>(), crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_6865616c7468))?;
    app = (::nagi_runtime::http_server::route(app, ::nagi_runtime::http_server::Method::GET, "/devices", ::nagi_runtime::http_server::public_policy::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_5374617465>(), crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_64657669636573))?;
    app = (::nagi_runtime::http_server::route(app, ::nagi_runtime::http_server::Method::GET, "/devices/{id}", ::nagi_runtime::http_server::public_policy::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_5374617465>(), crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_66696e645f646576696365))?;
    let mut port: ::std::primitive::i64 = (::nagi_runtime::parse_i64(&(match ::std::env::var("NAGI_SAMPLE_PORT") { ::std::result::Result::Ok(__nagi_env_value) => __nagi_env_value, ::std::result::Result::Err(_) => ("8091").to_owned() })))?;
    return (::nagi_runtime::http_server::serve(app, port, ::nagi_runtime::http_server::default_options())).await;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73716c6974655f6572726f72(mut problem: ::nagi_runtime::sqlite::Failure) -> ::nagi_runtime::Error {
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
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_6f70656e5f6461746162617365<'a>(mut path: &'a ::std::primitive::str) -> ::std::result::Result<::nagi_runtime::sqlite::Pool, ::nagi_runtime::Error> {
    let mut path: &::std::primitive::str = path;
    let mut config: ::nagi_runtime::sqlite::Options = (::nagi_runtime::sqlite::options(1i64, 2i64, 1000i64, 0i64))?;
    return ::nagi_runtime::result::map_error((::nagi_runtime::sqlite::open(path, config)).await, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73716c6974655f6572726f72);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73746f726167655f66696e645f646576696365<'a>(mut pool: &'a ::nagi_runtime::sqlite::Pool, mut value0: ::std::primitive::i64) -> ::std::result::Result<::std::option::Option<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_44657669636553657474696e6773>, ::nagi_runtime::Error> {
    let mut pool: &::nagi_runtime::sqlite::Pool = pool;
    let mut tx: ::nagi_runtime::sqlite::Tx = (::nagi_runtime::result::map_error((::nagi_runtime::sqlite::begin(pool, ::nagi_runtime::sqlite::BeginMode::Deferred)).await, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73716c6974655f6572726f72))?;
    let mut work: ::std::result::Result<::std::option::Option<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_44657669636553657474696e6773>, ::nagi_runtime::sqlite::Failure> = (::nagi_runtime::sqlite::query::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_44657669636553657474696e6773>(&(tx), ::nagi_runtime::sqlite::literal("SELECT id, enabled, gain, label, calibration FROM devices WHERE id = ?"), ::nagi_runtime::sqlite::bind_i64(::nagi_runtime::sqlite::parameters(), value0))).await;
    let mut ending: ::std::result::Result<(), ::nagi_runtime::sqlite::Failure> = (::nagi_runtime::sqlite::rollback(tx)).await;
    let mut value: ::std::option::Option<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_44657669636553657474696e6773> = (::nagi_runtime::result::map_error(work, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73716c6974655f6572726f72))?;
    (::nagi_runtime::result::map_error(ending, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73716c6974655f6572726f72))?;
    return ::std::result::Result::Ok(value);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73746f726167655f6c6973745f64657669636573<'a>(mut pool: &'a ::nagi_runtime::sqlite::Pool) -> ::std::result::Result<::std::vec::Vec<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_44657669636553657474696e6773>, ::nagi_runtime::Error> {
    let mut pool: &::nagi_runtime::sqlite::Pool = pool;
    let mut tx: ::nagi_runtime::sqlite::Tx = (::nagi_runtime::result::map_error((::nagi_runtime::sqlite::begin(pool, ::nagi_runtime::sqlite::BeginMode::Deferred)).await, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73716c6974655f6572726f72))?;
    let mut work: ::std::result::Result<::std::vec::Vec<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_44657669636553657474696e6773>, ::nagi_runtime::sqlite::Failure> = (::nagi_runtime::sqlite::all::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_44657669636553657474696e6773>(&(tx), ::nagi_runtime::sqlite::literal("SELECT id, enabled, gain, label, calibration FROM devices ORDER BY id"), ::nagi_runtime::sqlite::parameters())).await;
    let mut ending: ::std::result::Result<(), ::nagi_runtime::sqlite::Failure> = (::nagi_runtime::sqlite::rollback(tx)).await;
    let mut value: ::std::vec::Vec<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_44657669636553657474696e6773> = (::nagi_runtime::result::map_error(work, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73716c6974655f6572726f72))?;
    (::nagi_runtime::result::map_error(ending, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73716c6974655f6572726f72))?;
    return ::std::result::Result::Ok(value);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_626f6f7473747261705f64657669636573<'a>(mut pool: &'a ::nagi_runtime::sqlite::Pool) -> ::std::result::Result<::std::primitive::i64, ::nagi_runtime::Error> {
    let mut pool: &::nagi_runtime::sqlite::Pool = pool;
    let mut tx: ::nagi_runtime::sqlite::Tx = (::nagi_runtime::result::map_error((::nagi_runtime::sqlite::begin(pool, ::nagi_runtime::sqlite::BeginMode::Immediate)).await, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73716c6974655f6572726f72))?;
    let mut work: ::std::result::Result<::std::primitive::i64, ::nagi_runtime::sqlite::Failure> = (::nagi_runtime::sqlite::exec(&(tx), ::nagi_runtime::sqlite::literal("CREATE TABLE IF NOT EXISTS devices(id INTEGER PRIMARY KEY, enabled INTEGER, gain REAL, label TEXT, calibration BLOB)"), ::nagi_runtime::sqlite::parameters())).await;
    let mut ending: ::std::result::Result<(), ::nagi_runtime::sqlite::Failure> = (::nagi_runtime::sqlite::commit(tx)).await;
    let mut value: ::std::primitive::i64 = (::nagi_runtime::result::map_error(work, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73716c6974655f6572726f72))?;
    (::nagi_runtime::result::map_error(ending, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73716c6974655f6572726f72))?;
    return ::std::result::Result::Ok(value);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_736565645f64657669636573<'a>(mut pool: &'a ::nagi_runtime::sqlite::Pool) -> ::std::result::Result<::std::primitive::i64, ::nagi_runtime::Error> {
    let mut pool: &::nagi_runtime::sqlite::Pool = pool;
    let mut tx: ::nagi_runtime::sqlite::Tx = (::nagi_runtime::result::map_error((::nagi_runtime::sqlite::begin(pool, ::nagi_runtime::sqlite::BeginMode::Immediate)).await, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73716c6974655f6572726f72))?;
    let mut work: ::std::result::Result<::std::primitive::i64, ::nagi_runtime::sqlite::Failure> = (::nagi_runtime::sqlite::exec(&(tx), ::nagi_runtime::sqlite::literal("INSERT OR IGNORE INTO devices VALUES (1, 1, 1.25, NULL, X'0001FF'), (2, NULL, NULL, 'front-door', NULL), (3, 0, 0.5, 'side-door', X'')"), ::nagi_runtime::sqlite::parameters())).await;
    let mut ending: ::std::result::Result<(), ::nagi_runtime::sqlite::Failure> = (::nagi_runtime::sqlite::commit(tx)).await;
    let mut value: ::std::primitive::i64 = (::nagi_runtime::result::map_error(work, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73716c6974655f6572726f72))?;
    (::nagi_runtime::result::map_error(ending, crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73716c6974655f6572726f72))?;
    return ::std::result::Result::Ok(value);
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_44657669636553657474696e6773 as DeviceSettings;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_5374617465 as State;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_c_4572726f72426f6479 as ErrorBody;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_6465766963655f6964 as device_id;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_6d697373696e675f646576696365 as missing_device;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_6865616c7468 as health;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_66696e645f646576696365 as find_device;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_64657669636573 as devices;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73716c6974655f6572726f72 as sqlite_error;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_6f70656e5f6461746162617365 as open_database;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73746f726167655f66696e645f646576696365 as storage_find_device;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_73746f726167655f6c6973745f64657669636573 as storage_list_devices;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_626f6f7473747261705f64657669636573 as bootstrap_devices;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f6465766963652d73657474696e67732f6d61696e2e6e616769_f_736565645f64657669636573 as seed_devices;
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
