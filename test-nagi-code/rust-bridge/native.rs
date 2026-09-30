// This is an ordinary Rust module. Standard library, crates and isolated unsafe
// implementations can live here; the exposed interface is checked by rustc.
pub fn crc32(text: &str) -> i64 {
    let mut crc = u32::MAX;
    for &byte in text.as_bytes() {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    i64::from(!crc)
}

pub fn pretty_json(text: &str) -> Result<String, nagi_runtime::Error> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| nagi_runtime::Error::invalid(error.to_string()))?;
    serde_json::to_string_pretty(&value)
        .map_err(|error| nagi_runtime::Error::invalid(error.to_string()))
}

pub async fn double_later(value: i64) -> Result<i64, nagi_runtime::Error> {
    std::future::ready(Ok(value * 2)).await
}
