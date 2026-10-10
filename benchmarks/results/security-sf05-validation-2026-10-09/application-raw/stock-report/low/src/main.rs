#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize, Clone, Copy)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_c_53746f636b4c696e65 {
    pub product_id: ::std::primitive::i64,
    pub on_hand: ::std::primitive::i64,
    pub reserved: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_c_53746f636b4c696e65 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("StockLine").field("product_id", &self.product_id).field("on_hand", &self.on_hand).field("reserved", &self.reserved).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_c_53746f636b4c696e65 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["product_id","on_hand","reserved"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
product_id: row.get(ix[0])?,
on_hand: row.get(ix[1])?,
reserved: row.get(ix[2])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_c_4261746368 {
    pub warehouse: ::std::string::String,
    pub items: ::std::vec::Vec<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_c_53746f636b4c696e65>,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_c_4261746368 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Batch").field("warehouse", &self.warehouse).field("items", &self.items).finish()
    }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_c_5265706f7274 {
    pub warehouse: ::std::string::String,
    pub products: ::std::primitive::i64,
    pub available: ::std::primitive::i64,
    pub reserved: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_c_5265706f7274 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Report").field("warehouse", &self.warehouse).field("products", &self.products).field("available", &self.available).field("reserved", &self.reserved).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_c_5265706f7274 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["warehouse","products","available","reserved"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
warehouse: row.get(ix[0])?,
products: row.get(ix[1])?,
available: row.get(ix[2])?,
reserved: row.get(ix[3])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(Debug)]
pub enum __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_e_496e76656e746f72794572726f72 {
    InvalidJson {
        cause: ::nagi_runtime::Error,
    },
    TooManyProducts,
    InvalidWarehouse,
    InvalidProduct {
        index: ::std::primitive::i64,
    },
    InvalidCount {
        index: ::std::primitive::i64,
    },
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_f_73756d6d6172697a65<'a>(mut text: &'a ::std::primitive::str) -> ::std::result::Result<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_c_5265706f7274, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_e_496e76656e746f72794572726f72> {
    let mut text: &::std::primitive::str = text;
    let mut batch: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_c_4261746368 = (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_f_7061727365(text))?;
    if ((((((batch).warehouse).as_str()).len() as ::std::primitive::i64) < 1i64) || (((((batch).warehouse).as_str()).len() as ::std::primitive::i64) > 80i64)) {
        return ::std::result::Result::Err(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_e_496e76656e746f72794572726f72::InvalidWarehouse);
    }
    if (((((batch).items).as_slice()).len() as ::std::primitive::i64) > 10000i64) {
        return ::std::result::Result::Err(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_e_496e76656e746f72794572726f72::TooManyProducts);
    }
    let mut products: ::std::primitive::i64 = 0i64;
    let mut available: ::std::primitive::i64 = 0i64;
    let mut reserved: ::std::primitive::i64 = 0i64;
    for mut index in 0i64..((((batch).items).as_slice()).len() as ::std::primitive::i64) {
        if ((((batch).items)[::std::primitive::usize::try_from(index).expect("negative index")]).product_id < 1i64) {
            return ::std::result::Result::Err(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_e_496e76656e746f72794572726f72::InvalidProduct { index: products });
        }
        if (((((((batch).items)[::std::primitive::usize::try_from(index).expect("negative index")]).on_hand < 0i64) || ((((batch).items)[::std::primitive::usize::try_from(index).expect("negative index")]).on_hand > 1000000i64)) || ((((batch).items)[::std::primitive::usize::try_from(index).expect("negative index")]).reserved < 0i64)) || ((((batch).items)[::std::primitive::usize::try_from(index).expect("negative index")]).reserved > (((batch).items)[::std::primitive::usize::try_from(index).expect("negative index")]).on_hand)) {
            return ::std::result::Result::Err(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_e_496e76656e746f72794572726f72::InvalidCount { index: products });
        }
        products = (products + 1i64);
        available = (available + ((((batch).items)[::std::primitive::usize::try_from(index).expect("negative index")]).on_hand - (((batch).items)[::std::primitive::usize::try_from(index).expect("negative index")]).reserved));
        reserved = (reserved + (((batch).items)[::std::primitive::usize::try_from(index).expect("negative index")]).reserved);
    }
    return ::std::result::Result::Ok(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_c_5265706f7274 { warehouse: (((batch).warehouse).as_str()).to_owned(), products: products, available: available, reserved: reserved });
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_f_7061727365<'a>(mut text: &'a ::std::primitive::str) -> ::std::result::Result<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_c_4261746368, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_e_496e76656e746f72794572726f72> {
    let mut text: &::std::primitive::str = text;
    return ::nagi_runtime::result::map_error(::nagi_runtime::decode::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_c_4261746368>((&(text)).as_bytes()), crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_f_696e76616c69645f6a736f6e);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_f_696e76616c69645f6a736f6e(mut cause: ::nagi_runtime::Error) -> __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_e_496e76656e746f72794572726f72 {
    return __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_e_496e76656e746f72794572726f72::InvalidJson { cause: cause };
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_f_6d657373616765(mut problem: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_e_496e76656e746f72794572726f72) -> ::std::string::String {
    match problem {
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_e_496e76656e746f72794572726f72::InvalidJson { cause: _ } => {
            return ::std::string::String::from("input must contain a warehouse name and an items array with product_id, on_hand and reserved");
        },
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_e_496e76656e746f72794572726f72::InvalidWarehouse => {
            return ::std::string::String::from("warehouse must contain 1 to 80 UTF-8 bytes");
        },
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_e_496e76656e746f72794572726f72::TooManyProducts => {
            return ::std::string::String::from("input contains more than 10000 products");
        },
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_e_496e76656e746f72794572726f72::InvalidProduct { index: _ } => {
            return ::std::string::String::from("product_id must be positive");
        },
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_e_496e76656e746f72794572726f72::InvalidCount { index: _ } => {
            return ::std::string::String::from("counts must satisfy 0 <= reserved <= on_hand <= 1000000");
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_main() -> ::std::result::Result<(), ::nagi_runtime::Error> {
    let mut text: ::std::string::String = (::nagi_runtime::read_line())?;
    match crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_f_73756d6d6172697a65((text).as_str()) {
        ::std::result::Result::Ok(mut report) => {
            let mut encoded: ::std::string::String = (::nagi_runtime::encode(&report))?;
            return ::std::result::Result::Ok(println!("{}", encoded));
        },
        ::std::result::Result::Err(mut problem) => {
            return ::std::result::Result::Err(::nagi_runtime::Error::invalid(crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_f_6d657373616765(problem)));
        },
    }
}
#[allow(non_snake_case)]
pub mod inventory {
    #[allow(unused_imports)]
    pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_c_53746f636b4c696e65 as StockLine;
    #[allow(unused_imports)]
    pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_c_4261746368 as Batch;
    #[allow(unused_imports)]
    pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_c_5265706f7274 as Report;
    #[allow(unused_imports)]
    pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_e_496e76656e746f72794572726f72 as InventoryError;
    #[allow(unused_imports)]
    pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_f_73756d6d6172697a65 as summarize;
    #[allow(unused_imports)]
    pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_f_7061727365 as parse;
    #[allow(unused_imports)]
    pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_f_696e76616c69645f6a736f6e as invalid_json;
    #[allow(unused_imports)]
    pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6170706c69636174696f6e2d6578616d706c65732f73746f636b2d7265706f72742f696e76656e746f72792e6e616769_f_6d657373616765 as message;
}
fn main() {
if let ::std::result::Result::Err(e) = __nagi_main() { eprintln!("{}",e); ::std::process::exit(1); }
}
