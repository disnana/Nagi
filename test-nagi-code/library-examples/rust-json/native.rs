// The record is generated from serde-record.nagi in this same Rust crate.
pub fn parse_record(text: &str) -> Result<super::JsonRecord, nagi_runtime::Error> {
    serde_json::from_str(text).map_err(|error| nagi_runtime::Error::invalid(error.to_string()))
}
