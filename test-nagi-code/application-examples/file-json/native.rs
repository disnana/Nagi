use std::io::Write;

pub fn read_config_text(path: &str) -> Result<String, nagi_runtime::Error> {
    std::fs::read_to_string(path).map_err(|error| nagi_runtime::Error::invalid(error.to_string()))
}

pub fn write_new_config(path: &str, content: &str) -> Result<(), nagi_runtime::Error> {
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| nagi_runtime::Error::invalid(error.to_string()))?;
    output
        .write_all(content.as_bytes())
        .map_err(|error| nagi_runtime::Error::internal(error.to_string()))
}
