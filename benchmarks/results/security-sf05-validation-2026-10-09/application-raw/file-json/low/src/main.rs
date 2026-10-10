#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f66696c652d6a736f6e2f6d61696e2e6e616769_c_5361766564436f6e666967 {
    pub site: ::std::string::String,
    pub enabled: ::std::primitive::bool,
    pub retries: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f66696c652d6a736f6e2f6d61696e2e6e616769_c_5361766564436f6e666967 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("SavedConfig").field("site", &self.site).field("enabled", &self.enabled).field("retries", &self.retries).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f66696c652d6a736f6e2f6d61696e2e6e616769_c_5361766564436f6e666967 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["site","enabled","retries"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
site: row.get(ix[0])?,
enabled: row.get(ix[1])?,
retries: row.get(ix[2])?,
}) }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f66696c652d6a736f6e2f6d61696e2e6e616769_f_726561645f636f6e6669675f74657874<'a>(mut path: &'a ::std::primitive::str) -> ::std::result::Result<::std::string::String, ::nagi_runtime::Error> {
    native::read_config_text(path)
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f66696c652d6a736f6e2f6d61696e2e6e616769_f_77726974655f6e65775f636f6e666967<'a>(mut path: &'a ::std::primitive::str, mut content: &'a ::std::primitive::str) -> ::std::result::Result<(), ::nagi_runtime::Error> {
    native::write_new_config(path, content)
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_main() -> ::std::result::Result<(), ::nagi_runtime::Error> {
    let mut path: ::std::string::String = match ::std::env::var("NAGI_SAMPLE_FILE") { ::std::result::Result::Ok(__nagi_env_value) => __nagi_env_value, ::std::result::Result::Err(_) => ("configuration.json").to_owned() };
    let mut original: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f66696c652d6a736f6e2f6d61696e2e6e616769_c_5361766564436f6e666967 = __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f66696c652d6a736f6e2f6d61696e2e6e616769_c_5361766564436f6e666967 { site: ::std::string::String::from("東京"), enabled: true, retries: 3i64 };
    let mut encoded: ::std::string::String = (::nagi_runtime::encode(&original))?;
    (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f66696c652d6a736f6e2f6d61696e2e6e616769_f_77726974655f6e65775f636f6e666967((path).as_str(), (encoded).as_str()))?;
    let mut text: ::std::string::String = (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f66696c652d6a736f6e2f6d61696e2e6e616769_f_726561645f636f6e6669675f74657874((path).as_str()))?;
    let mut loaded: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f66696c652d6a736f6e2f6d61696e2e6e616769_c_5361766564436f6e666967 = (::nagi_runtime::decode::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f66696c652d6a736f6e2f6d61696e2e6e616769_c_5361766564436f6e666967>((&((text).as_str())).as_bytes()))?;
    assert!((((((loaded).site).as_str() == "東京") && (loaded).enabled) && ((loaded).retries == 3i64)));
    println!("{}", (::nagi_runtime::encode(&loaded))?);
    println!("{}", path);
    return ::std::result::Result::Ok(println!("{}", "file-json: OK"));
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f66696c652d6a736f6e2f6d61696e2e6e616769_c_5361766564436f6e666967 as SavedConfig;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f66696c652d6a736f6e2f6d61696e2e6e616769_f_726561645f636f6e6669675f74657874 as read_config_text;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f66696c652d6a736f6e2f6d61696e2e6e616769_f_77726974655f6e65775f636f6e666967 as write_new_config;
fn main() {
if let ::std::result::Result::Err(e) = __nagi_main() { eprintln!("{}",e); ::std::process::exit(1); }
}

#[path = "/workspace/Nagi-security-sf05/test-nagi-code/application-examples/file-json/native.rs"]
mod native;
