#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize, Clone, Copy)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_4c696e65 {
    pub product_id: ::std::primitive::i64,
    pub quantity: ::std::primitive::i64,
    pub unit_price_cents: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_4c696e65 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Line").field("product_id", &self.product_id).field("quantity", &self.quantity).field("unit_price_cents", &self.unit_price_cents).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_4c696e65 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["product_id","quantity","unit_price_cents"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
product_id: row.get(ix[0])?,
quantity: row.get(ix[1])?,
unit_price_cents: row.get(ix[2])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_4f72646572 {
    pub customer: ::std::string::String,
    pub items: ::std::vec::Vec<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_4c696e65>,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_4f72646572 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Order").field("customer", &self.customer).field("items", &self.items).finish()
    }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_51756f7465 {
    pub customer: ::std::string::String,
    pub lines: ::std::primitive::i64,
    pub units: ::std::primitive::i64,
    pub subtotal_cents: ::std::primitive::i64,
    pub discount_cents: ::std::primitive::i64,
    pub total_cents: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_51756f7465 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Quote").field("customer", &self.customer).field("lines", &self.lines).field("units", &self.units).field("subtotal_cents", &self.subtotal_cents).field("discount_cents", &self.discount_cents).field("total_cents", &self.total_cents).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_51756f7465 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["customer","lines","units","subtotal_cents","discount_cents","total_cents"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
customer: row.get(ix[0])?,
lines: row.get(ix[1])?,
units: row.get(ix[2])?,
subtotal_cents: row.get(ix[3])?,
discount_cents: row.get(ix[4])?,
total_cents: row.get(ix[5])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(Debug)]
pub enum __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72 {
    InvalidJson {
        cause: ::nagi_runtime::Error,
    },
    InvalidCustomer,
    InvalidLineCount,
    InvalidProduct,
    InvalidQuantity,
    InvalidPrice,
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_f_73756d6d6172697a65<'a>(mut text: &'a ::std::primitive::str) -> ::std::result::Result<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_51756f7465, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72> {
    let mut text: &::std::primitive::str = text;
    let mut order: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_4f72646572 = (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_f_7061727365(text))?;
    return crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_f_63616c63756c617465(order);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_f_7061727365<'a>(mut text: &'a ::std::primitive::str) -> ::std::result::Result<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_4f72646572, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72> {
    let mut text: &::std::primitive::str = text;
    match ::nagi_runtime::decode::<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_4f72646572>((&(text)).as_bytes()) {
        ::std::result::Result::Ok(mut order) => {
            return ::std::result::Result::Ok(order);
        },
        ::std::result::Result::Err(mut cause) => {
            return ::std::result::Result::Err(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72::InvalidJson { cause: cause });
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_f_63616c63756c617465(mut order: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_4f72646572) -> ::std::result::Result<__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_51756f7465, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72> {
    let mut customer_bytes: ::std::primitive::i64 = ((((order).customer).as_str()).len() as ::std::primitive::i64);
    if ((customer_bytes < 1i64) || (customer_bytes > 80i64)) {
        return ::std::result::Result::Err(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72::InvalidCustomer);
    }
    let mut lines: ::std::primitive::i64 = ((((order).items).as_slice()).len() as ::std::primitive::i64);
    if ((lines < 1i64) || (lines > 1000i64)) {
        return ::std::result::Result::Err(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72::InvalidLineCount);
    }
    let mut units: ::std::primitive::i64 = 0i64;
    let mut subtotal: ::std::primitive::i64 = 0i64;
    for mut line in ((order).items).iter().copied() {
        let mut line_total: ::std::primitive::i64 = (crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_f_7072696365((line).product_id, (line).quantity, (line).unit_price_cents))?;
        units = (units + (line).quantity);
        subtotal = (subtotal + line_total);
    }
    let mut discount: ::std::primitive::i64 = 0i64;
    if (subtotal >= 10000i64) {
        discount = (subtotal / 20i64);
    }
    return ::std::result::Result::Ok(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_51756f7465 { customer: (order).customer, lines: lines, units: units, subtotal_cents: subtotal, discount_cents: discount, total_cents: (subtotal - discount) });
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_f_7072696365(mut product_id: ::std::primitive::i64, mut quantity: ::std::primitive::i64, mut unit_price_cents: ::std::primitive::i64) -> ::std::result::Result<::std::primitive::i64, __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72> {
    if (product_id < 1i64) {
        return ::std::result::Result::Err(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72::InvalidProduct);
    }
    if ((quantity < 1i64) || (quantity > 1000i64)) {
        return ::std::result::Result::Err(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72::InvalidQuantity);
    }
    if ((unit_price_cents < 0i64) || (unit_price_cents > 1000000i64)) {
        return ::std::result::Result::Err(__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72::InvalidPrice);
    }
    return ::std::result::Result::Ok((quantity * unit_price_cents));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_f_6d657373616765(mut problem: __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72) -> ::std::string::String {
    match problem {
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72::InvalidJson { cause: _ } => {
            return ::std::string::String::from("input must contain customer and items with product_id, quantity and unit_price_cents");
        },
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72::InvalidCustomer => {
            return ::std::string::String::from("customer must contain 1 to 80 UTF-8 bytes");
        },
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72::InvalidLineCount => {
            return ::std::string::String::from("items must contain 1 to 1000 lines");
        },
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72::InvalidProduct => {
            return ::std::string::String::from("product_id must be positive");
        },
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72::InvalidQuantity => {
            return ::std::string::String::from("quantity must be between 1 and 1000");
        },
        __nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72::InvalidPrice => {
            return ::std::string::String::from("unit_price_cents must be between 0 and 1000000");
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_main() -> ::std::result::Result<(), ::nagi_runtime::Error> {
    let mut text: ::std::string::String = (::nagi_runtime::read_line())?;
    match crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_f_73756d6d6172697a65((text).as_str()) {
        ::std::result::Result::Ok(mut summary) => {
            let mut encoded: ::std::string::String = (::nagi_runtime::encode(&summary))?;
            return ::std::result::Result::Ok(println!("{}", encoded));
        },
        ::std::result::Result::Err(mut problem) => {
            return ::std::result::Result::Err(::nagi_runtime::Error::invalid(crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_f_6d657373616765(problem)));
        },
    }
}
#[allow(non_snake_case)]
pub mod quote {
    #[allow(unused_imports)]
    pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_4c696e65 as Line;
    #[allow(unused_imports)]
    pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_4f72646572 as Order;
    #[allow(unused_imports)]
    pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_c_51756f7465 as Quote;
    #[allow(unused_imports)]
    pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_e_51756f74654572726f72 as QuoteError;
    #[allow(unused_imports)]
    pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_f_73756d6d6172697a65 as summarize;
    #[allow(unused_imports)]
    pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_f_7061727365 as parse;
    #[allow(unused_imports)]
    pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_f_63616c63756c617465 as calculate;
    #[allow(unused_imports)]
    pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_f_7072696365 as price;
    #[allow(unused_imports)]
    pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d73656375726974792d736630352f746573742d6e6167692d636f64652f6c6f772d6578616d706c65732f6f726465722d71756f74652f71756f74652e6c6f77_f_6d657373616765 as message;
}
fn main() {
if let ::std::result::Result::Err(e) = __nagi_main() { eprintln!("{}",e); ::std::process::exit(1); }
}
